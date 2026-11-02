// The row of pinned apps and open windows on the bar, and where a drag on it lands. Every icon is
// a 24 px button 2 px apart, with a 1 px line between the pinned apps and the windows.
const STEP: f32 = 26.0;
const HALF: f32 = 12.0;
const LINE: f32 = 3.0;
// A window joins the pinned apps once it's over them; a pinned app leaves them only well past the
// last one, so nudging it doesn't unpin it.
const PIN_BEFORE: f32 = 13.0;
const UNPIN_PAST: f32 = 39.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Pinned(usize),
    Window(usize),
}

// What's under `x` in a row of `pinned` apps and `windows` open ones.
pub fn hit(x: f32, pinned: usize, windows: usize) -> Option<Hit> {
    let pinned_width = pinned as f32 * STEP;
    if x < 0.0 {
        return None;
    }
    if x < pinned_width {
        return Some(Hit::Pinned((x / STEP) as usize));
    }
    let start = if pinned > 0 && windows > 0 {
        pinned_width + LINE
    } else {
        pinned_width
    };
    let index = ((x - start).max(0.0) / STEP) as usize;
    (index < windows).then_some(Hit::Window(index))
}

// The pinned apps as they'd be if `dragged` (an app, pinned or not) were let go at `x`.
pub fn landing(pinned: &[String], dragged: &str, was_pinned: bool, x: f32) -> Vec<String> {
    let mut rest: Vec<String> = pinned.iter().filter(|p| *p != dragged).cloned().collect();
    let end = rest.len() as f32 * STEP;
    let stays = if was_pinned {
        x < end + UNPIN_PAST
    } else {
        x < end + PIN_BEFORE
    };
    if stays {
        let index = rest
            .iter()
            .enumerate()
            .filter(|(i, _)| *i as f32 * STEP + HALF < x)
            .count();
        rest.insert(index, dragged.to_owned());
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apps(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn a_press_finds_its_icon() {
        assert_eq!(hit(5.0, 2, 3), Some(Hit::Pinned(0)));
        assert_eq!(hit(30.0, 2, 3), Some(Hit::Pinned(1)));
        assert_eq!(hit(2.0 * STEP + LINE + 5.0, 2, 3), Some(Hit::Window(0)));
        assert_eq!(hit(5.0, 0, 3), Some(Hit::Window(0)));
        assert_eq!(hit(500.0, 2, 3), None);
    }

    #[test]
    fn a_pinned_app_moves_between_the_others() {
        let pinned = apps(&["a", "b", "c"]);
        assert_eq!(
            landing(&pinned, "a", true, 2.0 * STEP),
            apps(&["b", "c", "a"])
        );
        assert_eq!(landing(&pinned, "c", true, 0.0), apps(&["c", "a", "b"]));
        assert_eq!(
            landing(&pinned, "b", true, STEP + 4.0),
            pinned,
            "barely moved"
        );
    }

    #[test]
    fn dragged_well_past_the_others_it_is_unpinned() {
        let pinned = apps(&["a", "b", "c"]);
        assert_eq!(
            landing(&pinned, "c", true, 2.0 * STEP + 20.0),
            pinned,
            "a nudge"
        );
        assert_eq!(
            landing(&pinned, "c", true, 2.0 * STEP + 45.0),
            apps(&["a", "b"])
        );
    }

    #[test]
    fn a_window_dragged_among_the_pinned_apps_is_pinned_there() {
        let pinned = apps(&["a", "b"]);
        assert_eq!(landing(&pinned, "w", false, 20.0), apps(&["a", "w", "b"]));
        assert_eq!(
            landing(&pinned, "w", false, 2.0 * STEP + LINE + 10.0),
            pinned,
            "still among the windows"
        );
    }
}
