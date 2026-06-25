/// Last-uploaded state for the photodiode indicator — used to skip redundant
/// GPU uploads when nothing has changed.
#[derive(Default)]
pub struct PhotodiodeCache {
    pub enabled: bool,
    pub lit: Option<bool>,
    pub position: u32,
    pub screen_size: (u32, u32),
}
