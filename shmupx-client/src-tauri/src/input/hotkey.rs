/// Botão de controle vinculado ao push-to-talk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub device_id: String,
    pub button: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}
