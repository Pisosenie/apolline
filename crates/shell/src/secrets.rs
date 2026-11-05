use std::collections::HashMap;
use std::time::Duration;

use iced::futures::StreamExt;
use zbus::{Connection, ObjectPath, OwnedObjectPath, OwnedVariant, Variant};

// Passwords for CalDAV calendars and plugins' secret settings, kept by the Secret Service: GNOME
// Keyring, KeePassXC, KWallet.

const NONE: &str = "There's no keyring to keep the password in: start GNOME Keyring, KeePassXC or another Secret Service";
const SERVICE: &str = "org.freedesktop.secrets";
const SCHEMA: &str = "org.apolline.CalDav";
const PLUGIN_SCHEMA: &str = "org.apolline.Plugin";
// How long the keyring's own window may wait for an answer.
const PATIENCE: Duration = Duration::from_secs(120);

type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);

#[zbus::proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets"
)]
trait Service {
    fn open_session(
        &self,
        algorithm: &str,
        input: &Variant<'_>,
    ) -> zbus::Result<(OwnedVariant, OwnedObjectPath)>;
    fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> zbus::Result<(Vec<OwnedObjectPath>, Vec<OwnedObjectPath>)>;
    fn unlock(
        &self,
        objects: &[OwnedObjectPath],
    ) -> zbus::Result<(Vec<OwnedObjectPath>, OwnedObjectPath)>;
    fn get_secrets(
        &self,
        items: &[OwnedObjectPath],
        session: &ObjectPath<'_>,
    ) -> zbus::Result<HashMap<OwnedObjectPath, Secret>>;
    fn read_alias(&self, name: &str) -> zbus::Result<OwnedObjectPath>;
    fn create_collection(
        &self,
        properties: HashMap<&str, Variant<'_>>,
        alias: &str,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
}

#[zbus::proxy(
    interface = "org.freedesktop.Secret.Collection",
    default_service = "org.freedesktop.secrets"
)]
trait Collection {
    fn create_item(
        &self,
        properties: HashMap<&str, Variant<'_>>,
        secret: &Secret,
        replace: bool,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
}

#[zbus::proxy(
    interface = "org.freedesktop.Secret.Item",
    default_service = "org.freedesktop.secrets"
)]
trait Item {
    fn delete(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.Secret.Prompt",
    default_service = "org.freedesktop.secrets"
)]
trait Prompt {
    fn prompt(&self, window_id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn completed(&self, dismissed: bool, result: Variant<'_>) -> zbus::Result<()>;
}

// Whether there's a keyring, running or ready to start, so Settings can say so before asking for
// a password. Settings opens often, so it doesn't start one just to look.
pub async fn here() -> Result<(), String> {
    let connection = Connection::session().await.map_err(|_| NONE)?;
    let bus = zbus::fdo::DBusProxy::new(&connection)
        .await
        .map_err(|_| NONE)?;
    let name = zbus::names::BusName::try_from(SERVICE).map_err(said)?;
    let running = bus.name_has_owner(name).await.unwrap_or(false);
    let startable = bus
        .list_activatable_names()
        .await
        .unwrap_or_default()
        .iter()
        .any(|name| name.as_str() == SERVICE);
    if running || startable {
        Ok(())
    } else {
        Err(NONE.into())
    }
}

// Read in the background, so a locked keyring isn't asked to unlock; that waits for Settings.
pub async fn find(url: &str, user: &str) -> Result<Option<String>, String> {
    let keyring = Keyring::open().await?;
    keyring
        .read(attributes(url, user))
        .await?
        .ok_or_else(|| "The keyring is locked; the calendar follows once it's open".into())
}

pub async fn store(url: &str, user: &str, password: &str) -> Result<(), String> {
    let label = format!("Apolline calendar: {user} at {}", crate::events::shown(url));
    Keyring::open()
        .await?
        .write(label, attributes(url, user), password)
        .await
}

pub async fn forget(url: &str, user: &str) -> Result<(), String> {
    Keyring::open().await?.remove(attributes(url, user)).await
}

// A plugin's secret settings by key, those there are; a locked keyring or none gives nothing.
pub async fn plugin_find(plugin: &str, keys: &[String]) -> Vec<(String, String)> {
    let Ok(keyring) = Keyring::open().await else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for key in keys {
        if let Ok(Some(Some(value))) = keyring.read(plugin_attributes(plugin, key)).await {
            found.push((key.clone(), value));
        }
    }
    found
}

pub async fn plugin_store(plugin: &str, key: &str, value: &str) -> Result<(), String> {
    let label = format!("Apolline plugin {plugin}: {key}");
    let keyring = Keyring::open().await?;
    keyring
        .write(label, plugin_attributes(plugin, key), value)
        .await
}

pub async fn plugin_forget(plugin: &str, key: &str) -> Result<(), String> {
    Keyring::open()
        .await?
        .remove(plugin_attributes(plugin, key))
        .await
}

struct Keyring {
    connection: Connection,
    service: ServiceProxy<'static>,
    session: OwnedObjectPath,
}

impl Keyring {
    // "plain" sends the password as it is, over this user's own bus. The session ends with the
    // connection, so it's never closed by hand.
    async fn open() -> Result<Self, String> {
        let connection = Connection::session().await.map_err(|_| NONE)?;
        let service = ServiceProxy::new(&connection).await.map_err(|_| NONE)?;
        let (_, session) = service
            .open_session("plain", &Variant::from(""))
            .await
            .map_err(|_| NONE)?;
        Ok(Self {
            connection,
            service,
            session,
        })
    }

    // The secret under those attributes, or None while the keyring is locked.
    async fn read(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> Result<Option<Option<String>>, String> {
        let (unlocked, locked) = self.search(attributes).await?;
        if unlocked.is_empty() && !locked.is_empty() {
            return Ok(None);
        }
        let secrets = self
            .service
            .get_secrets(&unlocked, &self.session)
            .await
            .map_err(said)?;
        Ok(Some(
            unlocked
                .iter()
                .find_map(|item| secrets.get(item))
                .map(|(_, _, value, _)| String::from_utf8_lossy(value).into_owned()),
        ))
    }

    async fn write(
        &self,
        label: String,
        attributes: HashMap<&str, &str>,
        secret: &str,
    ) -> Result<(), String> {
        let collection = self.collection().await?;
        let properties = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Variant::from(label)),
            (
                "org.freedesktop.Secret.Item.Attributes",
                Variant::from(attributes),
            ),
        ]);
        let secret = (
            self.session.clone(),
            Vec::new(),
            secret.as_bytes().to_vec(),
            "text/plain".to_owned(),
        );
        let (_, prompt) = CollectionProxy::new(&self.connection, collection)
            .await
            .map_err(said)?
            .create_item(properties, &secret, true)
            .await
            .map_err(said)?;
        self.prompted(prompt).await.map(drop)
    }

    async fn remove(&self, attributes: HashMap<&str, &str>) -> Result<(), String> {
        let (mut items, locked) = self.search(attributes).await?;
        if !locked.is_empty() {
            self.unlock(&locked).await?;
            items.extend(locked);
        }
        for item in items {
            let prompt = ItemProxy::new(&self.connection, item)
                .await
                .map_err(said)?
                .delete()
                .await
                .map_err(said)?;
            self.prompted(prompt).await?;
        }
        Ok(())
    }

    async fn search(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> Result<(Vec<OwnedObjectPath>, Vec<OwnedObjectPath>), String> {
        self.service.search_items(attributes).await.map_err(said)
    }

    // The default collection, made if there's none, and unlocked. Either may ask the user.
    async fn collection(&self) -> Result<OwnedObjectPath, String> {
        let mut path = self.service.read_alias("default").await.map_err(said)?;
        if path.as_str() == "/" {
            let label = HashMap::from([(
                "org.freedesktop.Secret.Collection.Label",
                Variant::from("Login"),
            )]);
            let (made, prompt) = self
                .service
                .create_collection(label, "default")
                .await
                .map_err(said)?;
            path = match self.prompted(prompt).await? {
                Some(result) => OwnedObjectPath::try_from(result).map_err(said)?,
                None => made,
            };
        }
        self.unlock(std::slice::from_ref(&path)).await?;
        Ok(path)
    }

    async fn unlock(&self, paths: &[OwnedObjectPath]) -> Result<(), String> {
        let (_, prompt) = self.service.unlock(paths).await.map_err(said)?;
        self.prompted(prompt).await.map(drop)
    }

    // The keyring asking the user itself, for a password or a click. "/" means it needn't.
    async fn prompted(&self, prompt: OwnedObjectPath) -> Result<Option<OwnedVariant>, String> {
        if prompt.as_str() == "/" {
            return Ok(None);
        }
        let prompt = PromptProxy::new(&self.connection, prompt)
            .await
            .map_err(said)?;
        let mut done = prompt.receive_completed().await.map_err(said)?;
        prompt.prompt("").await.map_err(said)?;
        let signal = tokio::time::timeout(PATIENCE, done.next())
            .await
            .map_err(|_| "The keyring's window went unanswered")?
            .ok_or("The keyring went away")?;
        let args = signal.args().map_err(said)?;
        if args.dismissed {
            return Err("The keyring's window was closed without an answer".into());
        }
        OwnedVariant::try_from(args.result.clone())
            .map(Some)
            .map_err(said)
    }
}

fn attributes<'a>(url: &'a str, user: &'a str) -> HashMap<&'a str, &'a str> {
    HashMap::from([("xdg:schema", SCHEMA), ("url", url), ("user", user)])
}

fn plugin_attributes<'a>(plugin: &'a str, key: &'a str) -> HashMap<&'a str, &'a str> {
    HashMap::from([
        ("xdg:schema", PLUGIN_SCHEMA),
        ("plugin", plugin),
        ("key", key),
    ])
}

fn said(err: zbus::Error) -> String {
    crate::i18n::fill("The keyring said: {err}", &[("err", &err)])
}
