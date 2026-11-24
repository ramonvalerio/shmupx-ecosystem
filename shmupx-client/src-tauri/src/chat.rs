//! Chat da sala: links das mensagens abrem no navegador do sistema (nunca dentro do app).

/// Link aceito: `http`/`https`, sem espaços nem caracteres de controle, até 2048 caracteres.
fn link_valido(url: &str) -> bool {
    let resto = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    matches!(resto, Some(r) if !r.is_empty())
        && url.len() <= 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '<' || c == '>')
}

/// Abre no navegador do sistema um link de uma mensagem do chat.
#[tauri::command]
pub fn chat_abrir_link(url: String) -> Result<(), String> {
    if !link_valido(&url) {
        return Err("link inválido".into());
    }
    crate::auth::oauth::abrir_navegador(&url)
}

#[cfg(test)]
mod tests {
    use super::link_valido;

    #[test]
    fn so_links_web_validos() {
        assert!(link_valido("https://meet.google.com/chh-hdoi-uct"));
        assert!(link_valido("http://exemplo.com/a?b=1&c=2"));
        assert!(!link_valido("https://"));
        assert!(!link_valido("file:///C:/Windows"));
        assert!(!link_valido("javascript:alert(1)"));
        assert!(!link_valido("https://a.com/x y"));
        assert!(!link_valido("https://a.com/\"&calc"));
    }
}
