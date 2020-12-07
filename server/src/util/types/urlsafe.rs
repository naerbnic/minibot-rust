#[repr(transparent)]
pub struct UrlSafeStr(str);

#[repr(transparent)]
pub struct UrlSafeString(String);

impl std::ops::Deref for UrlSafeString {
    type Target = UrlSafeStr;
    fn deref(&self) -> &UrlSafeStr {
        // Transmutation is safe, as UrlSafeStr is declared as repr(transparent)
        unsafe { std::mem::transmute(&*self.0) }
    }
}
