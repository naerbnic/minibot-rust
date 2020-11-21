/// A secure string, with minimal attempts to prevent accidentally revealing it in logs or
/// configuration. Among the differences:
///
/// - Only provides deref access to the str, and not the string itself.
/// - Implements debug such that the contents are not revealed.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecureString(String);

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("SecureString(<{} bytes>)", self.0.len()))
    }
}

impl std::ops::Deref for SecureString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        SecureString(s)
    }
}

impl From<&'_ str> for SecureString {
    fn from(s: &'_ str) -> Self {
        SecureString(s.to_string())
    }
}
