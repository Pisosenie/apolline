use crate::Request;

// Where a window goes: a workspace by its id, its column counted from 1, and the column's width
// in percent. Below means into that column, under what's already there.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub workspace: u64,
    pub column: usize,
    pub width: Option<u8>,
    pub below: bool,
}

// What niri is asked, in this order, to put a window in its spot. niri only moves the focused
// column, so the window takes the focus for a moment and gives it back.
pub fn place(window: u64, spot: &Spot, focused: Option<u64>) -> Vec<Request> {
    let mut asked = vec![
        Request::MoveWindowToWorkspace {
            window,
            workspace: spot.workspace,
        },
        Request::FocusWindow(window),
    ];
    if spot.below {
        asked.push(Request::MoveColumnToIndex(spot.column + 1));
        asked.push(Request::ConsumeOrExpelWindowLeft(window));
    } else {
        asked.push(Request::MoveColumnToIndex(spot.column));
        asked.extend(spot.width.map(Request::SetColumnWidth));
    }
    asked.extend(focused.filter(|id| *id != window).map(Request::FocusWindow));
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_moves_takes_its_column_and_gives_the_focus_back() {
        let spot = Spot {
            workspace: 7,
            column: 2,
            width: Some(50),
            below: false,
        };
        let lines: Vec<String> = place(4, &spot, Some(9))
            .into_iter()
            .map(Request::line)
            .collect();
        assert_eq!(
            lines,
            [
                "{\"Action\":{\"MoveWindowToWorkspace\":{\"focus\":false,\"reference\":{\"Id\":7},\"window_id\":4}}}\n",
                "{\"Action\":{\"FocusWindow\":{\"id\":4}}}\n",
                "{\"Action\":{\"MoveColumnToIndex\":{\"index\":2}}}\n",
                "{\"Action\":{\"SetColumnWidth\":{\"change\":{\"SetProportion\":50}}}}\n",
                "{\"Action\":{\"FocusWindow\":{\"id\":9}}}\n",
            ]
        );
        let below = Spot {
            below: true,
            ..spot
        };
        assert_eq!(
            place(4, &below, Some(4)),
            [
                Request::MoveWindowToWorkspace {
                    window: 4,
                    workspace: 7
                },
                Request::FocusWindow(4),
                Request::MoveColumnToIndex(3),
                Request::ConsumeOrExpelWindowLeft(4),
            ]
        );
    }
}
