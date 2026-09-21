//! Where a GDTF Share account is kept — **S62**.
//!
//! # The decision, and why it is not `machine.json`
//!
//! `machine.json` is this desk's settings file: readable, copied between
//! machines, quoted in bug reports, and committed to a venue's backup. A
//! password in it would be a password in all of those. The owner chose the
//! operating system's own secret store on 2026-09-21, and that is what
//! [`Keychain`] is.
//!
//! **What is stored is the operator's own account with a third party**, not
//! anything of this desk's. It buys one thing — not typing a password every
//! time the library is updated — and it is opt-in: *remember me* is a box, and
//! [`Store::forget`] empties it again.
//!
//! # Platforms
//!
//! Windows has a credential manager and this uses it. **Linux and the
//! Raspberry Pi do not, here**: a desk in a rack has no logged-in desktop
//! session to unlock a keyring, and a store that silently fell back to a file
//! would be the plaintext this module exists to avoid. So on those platforms
//! remembering is refused, in words, and the operator types their password
//! when they update — which still works. `docs/FIXTURE_LIBRARY.md` §3 carries
//! that as the open half.
//!
//! # The seam
//!
//! [`Store`] is a trait for the same reason [`crate::share::Share`] is: the
//! real one cannot be exercised on a build machine, and everything that
//! *decides* should be testable. [`Remembered`] is the in-memory one the tests
//! use.

/// What went wrong, in words an operator can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    /// This platform has no secret store this desk will use.
    Unsupported,
    /// The store is there and refused, or could not be reached.
    Refused(String),
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(
                formatter,
                "this desk has no secret store on this platform, so the password is not kept"
            ),
            Self::Refused(why) => write!(formatter, "the secret store refused: {why}"),
        }
    }
}

impl std::error::Error for SecretError {}

/// One account, kept somewhere that is not a settings file.
pub trait Store {
    /// Writes the account, replacing what was there.
    ///
    /// # Errors
    ///
    /// [`SecretError::Unsupported`] where there is no store, and
    /// [`SecretError::Refused`] where there is one and it said no.
    fn remember(&mut self, user: &str, password: &str) -> Result<(), SecretError>;

    /// What was written, or `None` if nothing was.
    ///
    /// A store that is **not there** answers `None` rather than an error: a
    /// desk with no keyring has no remembered account, which is a fact and not
    /// a fault.
    fn recall(&mut self) -> Option<(String, String)>;

    /// Empties it. Answering `Ok` for an account that was not there, because
    /// *forget this* and *there was nothing* end in the same place.
    ///
    /// # Errors
    ///
    /// [`SecretError::Refused`].
    fn forget(&mut self) -> Result<(), SecretError>;
}

/// What the store is called, so an operator can find it in their own
/// credential manager and take it out by hand.
///
/// Windows-only because the store is: on a platform with no credential manager
/// there is no entry to name.
#[cfg(all(windows, not(test)))]
const SERVICE: &str = "PrismDMX — GDTF Share";

/// The same, for this crate's own tests — **a name of its own**, so running the
/// suite on an operator's machine can never overwrite or delete the account
/// they asked this desk to remember.
#[cfg(all(windows, test))]
const SERVICE: &str = "PrismDMX — GDTF Share (test)";

/// The entry's user field. The account name is the **secret**'s partner here,
/// so the slot itself is named for what it is rather than for who owns it.
#[cfg(windows)]
const ENTRY: &str = "gdtf-share-account";

/// The operating system's own secret store.
///
/// The one part of this module no test covers, for the reason the module
/// documentation gives. On Windows it is the credential manager; everywhere
/// else every call answers [`SecretError::Unsupported`] and [`Store::recall`]
/// answers `None`.
#[derive(Debug, Default)]
pub struct Keychain;

#[cfg(windows)]
impl Store for Keychain {
    fn remember(&mut self, user: &str, password: &str) -> Result<(), SecretError> {
        // The user name travels with the password, because the credential
        // manager keeps one secret per entry and this desk needs both.
        let entry = keyring::Entry::new(SERVICE, ENTRY).map_err(from_keyring)?;
        entry
            .set_password(&format!("{user}\n{password}"))
            .map_err(from_keyring)
    }

    fn recall(&mut self) -> Option<(String, String)> {
        let entry = keyring::Entry::new(SERVICE, ENTRY).ok()?;
        let kept = entry.get_password().ok()?;
        let (user, password) = kept.split_once('\n')?;
        Some((user.to_owned(), password.to_owned()))
    }

    fn forget(&mut self) -> Result<(), SecretError> {
        let entry = match keyring::Entry::new(SERVICE, ENTRY) {
            Ok(entry) => entry,
            // Nothing to take out.
            Err(_) => return Ok(()),
        };
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(from_keyring(error)),
        }
    }
}

/// Which of this module's two faults a `keyring` error is — **S62**.
///
/// `NoDefaultStore` is the one worth separating: it means the platform's own
/// credential store could not be initialised at all, which is *this desk
/// cannot keep a password here* and not *the store said no*. An operator told
/// the wrong one of those would go looking in the wrong place.
#[cfg(windows)]
fn from_keyring(error: keyring::Error) -> SecretError {
    match error {
        keyring::Error::NoDefaultStore => SecretError::Unsupported,
        other => SecretError::Refused(other.to_string()),
    }
}

#[cfg(not(windows))]
impl Store for Keychain {
    fn remember(&mut self, _user: &str, _password: &str) -> Result<(), SecretError> {
        Err(SecretError::Unsupported)
    }

    fn recall(&mut self) -> Option<(String, String)> {
        None
    }

    fn forget(&mut self) -> Result<(), SecretError> {
        // There was nothing to forget, which is the state asked for.
        Ok(())
    }
}

/// A store that keeps one account in memory — for tests, and for a desk that
/// is told to remember on a platform that cannot.
///
/// It is **not** a fallback that gets used by accident: nothing constructs one
/// but a test and the caller that means to.
#[derive(Debug, Default)]
pub struct Remembered {
    /// What was written, if anything.
    pub account: Option<(String, String)>,
    /// Set when this store is meant to refuse, so a caller's handling of a
    /// refusal can be driven.
    pub refuse: bool,
}

impl Store for Remembered {
    fn remember(&mut self, user: &str, password: &str) -> Result<(), SecretError> {
        if self.refuse {
            return Err(SecretError::Unsupported);
        }
        self.account = Some((user.to_owned(), password.to_owned()));
        Ok(())
    }

    fn recall(&mut self) -> Option<(String, String)> {
        self.account.clone()
    }

    fn forget(&mut self) -> Result<(), SecretError> {
        self.account = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Keychain, Remembered, SecretError, Store};

    /// What a caller is promised, over the store a test can drive.
    #[test]
    fn an_account_is_written_read_back_and_taken_out_again() {
        let mut store = Remembered::default();
        assert_eq!(store.recall(), None, "nothing is remembered to begin with");

        store.remember("somebody", "a secret").expect("it writes");
        assert_eq!(
            store.recall(),
            Some(("somebody".to_owned(), "a secret".to_owned()))
        );

        // Writing again replaces rather than adds.
        store
            .remember("somebody", "a different one")
            .expect("it writes");
        assert_eq!(
            store.recall(),
            Some(("somebody".to_owned(), "a different one".to_owned()))
        );

        store.forget().expect("it empties");
        assert_eq!(store.recall(), None);
        store.forget().expect("forgetting twice is not an error");
    }

    /// A store that will not keep anything says so rather than pretending.
    #[test]
    fn a_store_that_refuses_is_an_error_and_not_a_silent_loss() {
        let mut store = Remembered {
            refuse: true,
            ..Remembered::default()
        };
        assert_eq!(
            store.remember("somebody", "a secret"),
            Err(SecretError::Unsupported)
        );
        assert_eq!(store.recall(), None);
    }

    /// **On a platform with no store, remembering is refused in words and
    /// recalling is simply nothing.**
    ///
    /// The second half is the one that matters: a desk with no keyring has no
    /// remembered account, which is a fact rather than a fault, and a caller
    /// that treated it as one would put an error in front of an operator every
    /// time they opened the panel.
    #[cfg(not(windows))]
    #[test]
    fn a_platform_without_a_store_refuses_to_remember_and_recalls_nothing() {
        let mut store = Keychain;
        assert_eq!(
            store.remember("somebody", "a secret"),
            Err(SecretError::Unsupported)
        );
        assert_eq!(store.recall(), None);
        store.forget().expect("there is nothing to take out");
    }

    /// The real store is constructible wherever this compiles.
    #[test]
    fn the_platform_store_exists() {
        let _ = Keychain;
    }

    /// **The Windows credential manager, for real** — added when S62 was
    /// reviewed on Windows, because the module's first draft named this as the
    /// one part no test covered and the release waits on it working.
    ///
    /// Every step goes through a **fresh** [`Keychain`], so what is asserted is
    /// that the account is in the operating system's store and not in a value
    /// this process happens to hold — the failure a mock-backed store would
    /// have. `cmdkey /list` is asked as well: an entry the credential manager
    /// itself lists is one an operator can find and remove by hand, which is
    /// what the module documentation promises.
    ///
    /// It uses the test service name, never the operator's (see `SERVICE`).
    #[cfg(windows)]
    #[test]
    fn the_windows_credential_manager_keeps_forgets_and_lists_the_account() {
        let listed = || {
            let output = std::process::Command::new("cmdkey")
                .arg("/list")
                .output()
                .expect("cmdkey is part of Windows");
            // `gdtf-share-account.PrismDMX — GDTF Share (test)`, as a generic
            // credential. Matched on the entry **and** the test suffix, so an
            // operator's own remembered account never counts; the dash is
            // left out because `cmdkey` prints it in the console's code page.
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.contains(super::ENTRY) && line.contains("GDTF Share (test)"))
        };

        Keychain.forget().expect("a clean start");
        assert_eq!(Keychain.recall(), None, "nothing is kept to begin with");

        // A password with the characters a real one has: spaces, umlauts, the
        // separator this module writes between user and password.
        Keychain
            .remember(
                "operator@venue.example",
                "Pässwort mit
Zeile & €",
            )
            .expect("the credential manager keeps it");
        assert_eq!(
            Keychain.recall(),
            Some((
                "operator@venue.example".to_owned(),
                "Pässwort mit
Zeile & €"
                    .to_owned()
            )),
            "a second handle reads back what the first wrote"
        );
        assert!(listed(), "the credential manager lists the entry");

        // Remembering again replaces rather than adds a second entry.
        Keychain
            .remember("operator@venue.example", "neu")
            .expect("it replaces");
        assert_eq!(
            Keychain.recall().map(|(_, password)| password).as_deref(),
            Some("neu")
        );

        Keychain.forget().expect("it is taken out");
        assert_eq!(Keychain.recall(), None, "forgotten is gone");
        assert!(!listed(), "and the credential manager no longer lists it");
        Keychain.forget().expect("forgetting twice is not an error");
    }
}
