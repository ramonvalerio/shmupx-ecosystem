//! Configuração do ShmupX no MiSTer: `<pasta>/shmupx.ini`, no formato `chave = valor`.
//!
//! ```ini
//! username = [ramonvalerio, dantrius, kwayd, convidado]
//! ```
//!
//! Cada username vira uma pasta de jogador no menu (ShmupX > RAMONVALERIO > Hi-Score > ...).
//! O `.mgl` de cada pasta abre o core com `setname` = `MegaDrive_<id>`, e o MiSTer grava esse
//! nome em `/tmp/CORENAME`: é assim que o serviço sabe quem está jogando.

use std::path::Path;

const ARQUIVO: &str = "shmupx.ini";
pub const JOGADOR_PADRAO: &str = "JOGADOR";
const TAMANHO_NOME: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usuario {
    /// Para o `setname` do `.mgl` e nomes de arquivo: `ramonvalerio`.
    pub id: String,
    /// Para o menu e o ranking: `RAMONVALERIO`.
    pub nome: String,
}

impl Usuario {
    pub fn novo(texto: &str) -> Option<Self> {
        let nome = normalizar_nome(texto);
        let id: String = nome
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();
        (!id.is_empty()).then_some(Self { id, nome })
    }
}

/// Usuários na ordem do ini. `username = nome` ou `username = [a, b, c]`.
pub fn usuarios(pasta: &Path) -> Vec<Usuario> {
    let valor = std::fs::read_to_string(pasta.join(ARQUIVO)).ok().and_then(|texto| ler_chave(&texto, "username"));
    let mut lista: Vec<Usuario> = Vec::new();
    for parte in valor.as_deref().unwrap_or("").trim_matches(|c| c == '[' || c == ']').split(',') {
        if let Some(usuario) = Usuario::novo(parte)
            && !lista.iter().any(|u| u.id == usuario.id)
        {
            lista.push(usuario);
        }
    }
    lista
}

/// Modos do ShmupX: (token no nome do core, id gravado na partida). O MiSTer corta o nome
/// do core em 31 caracteres, por isso os tokens são curtos; os longos são da versão anterior.
pub const MODOS: [(&str, &str); 8] = [
    ("hs", "hi-score"),
    ("1cc", "1cc"),
    ("spr", "caravan-sprint"),
    ("cls", "caravan-classic"),
    ("hiscore", "hi-score"),
    ("sprint", "caravan-sprint"),
    ("classic", "caravan-classic"),
    ("hiscor", "hi-score"),
];

/// Quem está jogando e em que modo, pelo nome do core que o menu ShmupX passa no `setname`:
/// `MegaDrive_kwayd_sprint`, `dariusg_ramonvalerio_hiscore`. Sem o jogador (core aberto fora
/// do menu ShmupX) ou com um id desconhecido, vale o primeiro usuário do ini.
pub fn identificar(pasta: &Path, core: &str) -> (Usuario, Option<&'static str>) {
    let mut partes: Vec<String> = core.split('_').skip(1).map(str::to_ascii_lowercase).collect();
    let modo = partes
        .last()
        .and_then(|token| MODOS.iter().find(|(t, _)| t == token))
        .map(|(_, modo)| *modo);
    if modo.is_some() {
        partes.pop();
    }
    let lista = usuarios(pasta);
    let usuario = partes
        .last()
        .and_then(|id| lista.iter().find(|u| &u.id == id))
        .or_else(|| lista.first())
        .cloned()
        .unwrap_or_else(|| Usuario { id: JOGADOR_PADRAO.to_ascii_lowercase(), nome: JOGADOR_PADRAO.to_owned() });
    (usuario, modo)
}

/// Core aberto por um item do menu ShmupX (traz jogador e modo no nome).
pub fn core_do_shmupx(core: &str) -> bool {
    core.rsplit('_').next().is_some_and(|token| MODOS.iter().any(|(t, _)| *t == token.to_ascii_lowercase()))
}

const ARQUIVO_ULTIMO: &str = "ultimo_jogador";

/// Guarda quem jogou por último, para essa pasta subir ao topo do menu.
pub fn registrar_ultimo(pasta: &Path, usuario: &Usuario) -> std::io::Result<()> {
    std::fs::write(pasta.join(ARQUIVO_ULTIMO), format!("{}\n", usuario.id))
}

/// Ordem do menu: quem jogou por último primeiro, depois os outros na ordem do ini.
pub fn usuarios_em_ordem(pasta: &Path) -> Vec<Usuario> {
    let mut lista = usuarios(pasta);
    let ultimo = std::fs::read_to_string(pasta.join(ARQUIVO_ULTIMO)).unwrap_or_default();
    if let Some(posicao) = lista.iter().position(|u| u.id == ultimo.trim()) {
        let usuario = lista.remove(posicao);
        lista.insert(0, usuario);
    }
    lista
}

/// Endereço do backend (`api = https://...`). Sem ele, as partidas ficam só no ranking local.
pub fn api(pasta: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(pasta.join(ARQUIVO)).ok()?;
    ler_chave(&texto, "api").map(|url| url.trim_end_matches('/').to_owned()).filter(|url| url.starts_with("http"))
}

/// Token do MiSTer do jogador (`token.<id> = sxm_...`), recebido por e-mail pelo Client.
pub fn token(pasta: &Path, usuario: &Usuario) -> Option<String> {
    let texto = std::fs::read_to_string(pasta.join(ARQUIVO)).ok()?;
    ler_chave(&texto, &format!("token.{}", usuario.id)).filter(|t| t.starts_with("sxm_"))
}

/// Primeiro usuário do ini (dono do MiSTer), para partidas antigas sem jogador.
pub fn jogador_principal(pasta: &Path) -> String {
    usuarios(pasta).into_iter().next().map_or_else(|| JOGADOR_PADRAO.to_owned(), |u| u.nome)
}

/// Maiúsculas, só letras/números/espaço/hífen (a fonte do MiSTer não tem acentos) e no
/// máximo 12 caracteres, para caber na janela do menu Scripts.
pub fn normalizar_nome(nome: &str) -> String {
    let limpo: String = nome
        .trim()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'é' | 'ê' | 'è' | 'É' | 'Ê' | 'È' => 'E',
            'í' | 'Í' => 'I',
            'ó' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ü' | 'Ú' | 'Ü' => 'U',
            'ç' | 'Ç' => 'C',
            c => c,
        })
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-')
        .collect();
    limpo.to_ascii_uppercase().chars().take(TAMANHO_NOME).collect::<String>().trim().to_owned()
}

fn ler_chave(texto: &str, chave: &str) -> Option<String> {
    texto.lines().find_map(|linha| {
        let (k, v) = linha.split_once('=')?;
        (k.trim() == chave).then(|| v.trim().to_owned())
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_com_ini(nome: &str, ini: &str) -> std::path::PathBuf {
        let pasta = std::env::temp_dir().join(format!("shmupx-config-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join(ARQUIVO), ini).unwrap();
        pasta
    }

    #[test]
    fn normaliza_o_nome_para_a_tela_do_mister() {
        assert_eq!(normalizar_nome("ramonvalerio"), "RAMONVALERIO");
        assert_eq!(normalizar_nome("  João da Silva Santos "), "JOAO DA SILV");
        assert_eq!(normalizar_nome("!!!"), "");
    }

    #[test]
    fn le_lista_de_usernames() {
        let pasta = pasta_com_ini("lista", "# jogadores\nusername = [ramonvalerio, dantrius, kwayd, convidado, Dantrius, ]\n");

        let ids: Vec<String> = usuarios(&pasta).into_iter().map(|u| u.id).collect();

        assert_eq!(ids, ["ramonvalerio", "dantrius", "kwayd", "convidado"]);
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn aceita_um_username_so() {
        let pasta = pasta_com_ini("um", "username = ramonvalerio\n");

        assert_eq!(usuarios(&pasta), vec![Usuario { id: "ramonvalerio".into(), nome: "RAMONVALERIO".into() }]);
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn identifica_o_jogador_pelo_nome_do_core() {
        let pasta = pasta_com_ini("core", "username = [ramonvalerio, kwayd]\n");

        let nome_e_modo = |core: &str| {
            let (usuario, modo) = identificar(&pasta, core);
            (usuario.nome, modo)
        };
        assert_eq!(nome_e_modo("SX_kwayd_spr"), ("KWAYD".into(), Some("caravan-sprint")));
        assert_eq!(nome_e_modo("SX_ramonvalerio_cls"), ("RAMONVALERIO".into(), Some("caravan-classic")));
        assert_eq!(nome_e_modo("ket_ramonvalerio_hs"), ("RAMONVALERIO".into(), Some("hi-score")));
        assert_eq!(nome_e_modo("MegaDriveRA_ramonvalerio_hiscor"), ("RAMONVALERIO".into(), Some("hi-score")));
        assert_eq!(nome_e_modo("MegaDrive_kwayd_sprint"), ("KWAYD".into(), Some("caravan-sprint")));
        assert_eq!(nome_e_modo("dariusg_kwayd_classic"), ("KWAYD".into(), Some("caravan-classic")));
        assert_eq!(nome_e_modo("MegaDrive_ramonvalerio_hiscore"), ("RAMONVALERIO".into(), Some("hi-score")));
        assert_eq!(nome_e_modo("dariusg_kwayd_1cc"), ("KWAYD".into(), Some("1cc")));
        assert_eq!(nome_e_modo("MegaDrive_kwayd"), ("KWAYD".into(), None)); // formato antigo
        assert_eq!(nome_e_modo("MegaDrive"), ("RAMONVALERIO".into(), None));
        assert_eq!(nome_e_modo("MegaDrive_sumiu_hiscore"), ("RAMONVALERIO".into(), Some("hi-score")));
        assert!(core_do_shmupx("dariusg_kwayd_sprint"));
        assert!(!core_do_shmupx("dariusg"));
        assert!(!core_do_shmupx("MENU"));
        std::fs::remove_dir_all(&pasta).unwrap();

        let vazia = std::env::temp_dir().join(format!("shmupx-config-vazia-{}", std::process::id()));
        assert_eq!(identificar(&vazia, "MegaDrive_kwayd_hiscore").0.nome, JOGADOR_PADRAO);
    }

    #[test]
    fn le_api_e_token_por_jogador() {
        let pasta = pasta_com_ini(
            "token",
            "username = [ramonvalerio, kwayd]\napi = https://api.shmupx.example/\ntoken.ramonvalerio = sxm_abc\ntoken.kwayd = errado\n",
        );
        let (ramon, kwayd) = (identificar(&pasta, "SX_ramonvalerio_hs").0, identificar(&pasta, "SX_kwayd_hs").0);

        assert_eq!(api(&pasta).as_deref(), Some("https://api.shmupx.example"));
        assert_eq!(token(&pasta, &ramon).as_deref(), Some("sxm_abc"));
        assert_eq!(token(&pasta, &kwayd), None); // sem o prefixo sxm_
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn ultimo_a_jogar_vai_para_o_topo_e_o_resto_segue_o_ini() {
        let pasta = pasta_com_ini("ordem", "username = [ramonvalerio, dantrius, kwayd, convidado]\n");
        let ordem = |pasta: &Path| usuarios_em_ordem(pasta).into_iter().map(|u| u.nome).collect::<Vec<_>>();
        assert_eq!(ordem(&pasta), ["RAMONVALERIO", "DANTRIUS", "KWAYD", "CONVIDADO"]);

        registrar_ultimo(&pasta, &identificar(&pasta, "MegaDrive_kwayd_hiscore").0).unwrap();
        assert_eq!(ordem(&pasta), ["KWAYD", "RAMONVALERIO", "DANTRIUS", "CONVIDADO"]);

        registrar_ultimo(&pasta, &identificar(&pasta, "dariusg_convidado_sprint").0).unwrap();
        assert_eq!(ordem(&pasta), ["CONVIDADO", "RAMONVALERIO", "DANTRIUS", "KWAYD"]);
        std::fs::remove_dir_all(pasta).unwrap();
    }
}
