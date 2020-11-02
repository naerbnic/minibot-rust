
    use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug)]
    pub struct OAuthScopeList(Vec<String>);

    fn is_valid_scope(scope: &str) -> bool {
        // This follows the spec for "scope-token" at
        // https://tools.ietf.org/html/rfc6749#section-3.3
        if scope.is_empty() {
            return false;
        }

        for ch in scope.chars() {
            let ch_code: u32 = ch.into();
            let valid_char = (ch_code == 0x21)
                || (ch_code >= 0x23 && ch_code < 0x5B)
                || (ch_code >= 0x5D && ch_code < 0x7E);
            if !valid_char {
                return false;
            }
        }

        return true;
    }

    impl OAuthScopeList {
        fn from_vec(vec: Vec<String>) -> Option<Self> {
            for entry in &vec {
                if !is_valid_scope(entry) {
                    return None;
                }
            }

            Some(OAuthScopeList(vec))
        }

        fn new_empty() -> Self {
            OAuthScopeList(vec![])
        }

        /// Creates a new OAuthScopes from strings. This is intended to be used
        /// with literals. Panics if an invalid scope is passed in.
        fn new(scopes: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
            let mut vec = Vec::new();
            for scope in scopes.into_iter() {
                let scope = scope.as_ref();
                assert!(is_valid_scope(scope));
                vec.push(scope.to_string());
            }
            OAuthScopeList(vec)
        }

        fn scopes(&self) -> &Vec<String> {
            &self.0
        }
    }

    impl Serialize for OAuthScopeList {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0.join(" "))
        }
    }

    impl<'de> Deserialize<'de> for OAuthScopeList {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let scopes_str: &str = Deserialize::deserialize(deserializer)?;
            log::debug!("Deserialized scopes: {:?}", scopes_str);
            if scopes_str.is_empty() {
                return Ok(OAuthScopeList::new_empty());
            }
            let vec = scopes_str
                .split(' ')
                .map(str::to_string)
                .collect::<Vec<_>>();
            OAuthScopeList::from_vec(vec).ok_or_else(|| D::Error::custom("Invalid scope names"))
        }
    }