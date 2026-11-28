//! Leitura do `appcache/appinfo.vdf` da Steam (binário): de cada jogo, como a Steam o abre
//! (`appinfo > config > launch`). Serve para abrir direto, sem a Steam, os jogos sem a proteção
//! SteamStub (flag No Steam Protection do Lab): o executável aberto é o mesmo que a Steam abriria.
//!
//! Formato (versões 27, 28 e 29 do arquivo): cabeçalho (`magic`, universo e, na 29, a posição da
//! tabela de textos), depois um registro por app (`appid`, tamanho, dados fixos e o KeyValues
//! binário) até um `appid` 0. No KeyValues binário cada item é um tipo (1 byte), a chave (texto
//! terminado em 0 ou, na versão 29, o índice na tabela de textos) e o valor.

const V27: u32 = 0x0756_4427;
const V28: u32 = 0x0756_4428;
const V29: u32 = 0x0756_4429;
/// Profundidade máxima do KeyValues (o `launch` fica no nível 3): arquivo corrompido não estoura a pilha.
const PROFUNDIDADE_MAXIMA: usize = 32;

#[derive(Debug, Clone, PartialEq)]
enum Valor {
    Texto(String),
    Numero(i64),
    Mapa(Vec<(String, Valor)>),
}

impl Valor {
    fn filho(&self, chave: &str) -> Option<&Valor> {
        match self {
            Valor::Mapa(itens) => itens.iter().find(|(k, _)| k.eq_ignore_ascii_case(chave)).map(|(_, v)| v),
            _ => None,
        }
    }

    fn texto(&self, chave: &str) -> String {
        match self.filho(chave) {
            Some(Valor::Texto(t)) => t.clone(),
            Some(Valor::Numero(n)) => n.to_string(),
            _ => String::new(),
        }
    }
}

struct Leitor<'a> {
    dados: &'a [u8],
    pos: usize,
    textos: Option<Vec<String>>,
}

impl<'a> Leitor<'a> {
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let fim = self.pos.checked_add(n)?;
        let parte = self.dados.get(self.pos..fim)?;
        self.pos = fim;
        Some(parte)
    }

    fn u8(&mut self) -> Option<u8> {
        self.bytes(1).map(|b| b[0])
    }

    fn u32(&mut self) -> Option<u32> {
        self.bytes(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Option<u64> {
        let b = self.bytes(8)?;
        Some(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn texto_terminado(&mut self) -> Option<String> {
        let resto = self.dados.get(self.pos..)?;
        let fim = resto.iter().position(|&b| b == 0)?;
        let texto = String::from_utf8_lossy(&resto[..fim]).into_owned();
        self.pos += fim + 1;
        Some(texto)
    }

    fn chave(&mut self) -> Option<String> {
        match &self.textos {
            Some(_) => {
                let indice = self.u32()? as usize;
                self.textos.as_ref()?.get(indice).cloned()
            }
            None => self.texto_terminado(),
        }
    }

    /// Um mapa do KeyValues binário, até o tipo 8 (fim).
    fn mapa(&mut self, profundidade: usize) -> Option<Valor> {
        if profundidade > PROFUNDIDADE_MAXIMA {
            return None;
        }
        let mut itens = Vec::new();
        loop {
            let tipo = self.u8()?;
            if tipo == 8 || tipo == 11 {
                return Some(Valor::Mapa(itens));
            }
            let chave = self.chave()?;
            let valor = match tipo {
                0 => self.mapa(profundidade + 1)?,
                1 => Valor::Texto(self.texto_terminado()?),
                2 => Valor::Numero(i64::from(self.u32()? as i32)),
                // float, ponteiro e cor: 4 bytes que não usamos.
                3 | 4 | 6 => {
                    self.bytes(4)?;
                    Valor::Numero(0)
                }
                7 => Valor::Numero(self.u64()? as i64),
                10 => Valor::Numero(self.u64()? as i64),
                _ => return None,
            };
            itens.push((chave, valor));
        }
    }
}

/// Uma forma de abrir o jogo cadastrada na Steam (`config > launch > <n>`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lancamento {
    pub executavel: String,
    pub argumentos: String,
    pub pasta_de_trabalho: String,
    /// `default` (ou vazio) é o "Jogar"; `config`, `option1`… são as outras opções do menu.
    pub tipo: String,
    /// Sistemas (`windows`, `linux`, `macos`, separados por vírgula); vazio = todos.
    pub sistemas: String,
    /// Só para quem está num beta da Steam.
    pub beta: bool,
}

/// Tabela de textos da versão 29 (as chaves do KeyValues são índices nela).
fn tabela_de_textos(dados: &[u8], posicao: usize) -> Option<Vec<String>> {
    let mut leitor = Leitor { dados, pos: posicao, textos: None };
    let quantidade = leitor.u32()? as usize;
    let mut textos = Vec::with_capacity(quantidade.min(1 << 20));
    for _ in 0..quantidade {
        textos.push(leitor.texto_terminado()?);
    }
    Some(textos)
}

/// As formas de abrir o app `app_id` registradas no `appinfo.vdf` (`dados`), na ordem da Steam.
/// `None`: arquivo de versão desconhecida, corrompido ou sem o app.
pub fn lancamentos(dados: &[u8], app_id: u32) -> Option<Vec<Lancamento>> {
    let mut leitor = Leitor { dados, pos: 0, textos: None };
    let versao = leitor.u32()?;
    if ![V27, V28, V29].contains(&versao) {
        return None;
    }
    leitor.u32()?; // universo
    if versao == V29 {
        let posicao = usize::try_from(leitor.u64()?).ok()?;
        leitor.textos = Some(tabela_de_textos(dados, posicao)?);
    }
    // Dados fixos de cada registro antes do KeyValues: estado, atualização, token, SHA-1, mudança
    // e, a partir da 28, o SHA-1 do binário.
    let fixos = 4 + 4 + 8 + 20 + 4 + if versao >= V28 { 20 } else { 0 };
    loop {
        let id = leitor.u32()?;
        if id == 0 {
            return None;
        }
        let tamanho = leitor.u32()? as usize;
        let inicio = leitor.pos;
        let fim = inicio.checked_add(tamanho)?;
        if fim > dados.len() {
            return None;
        }
        if id != app_id {
            leitor.pos = fim;
            continue;
        }
        leitor.pos = inicio.checked_add(fixos)?;
        let raiz = leitor.mapa(0)?;
        let launch = raiz.filho("appinfo").unwrap_or(&raiz).filho("config")?.filho("launch")?;
        let Valor::Mapa(itens) = launch else { return None };
        let mut lista: Vec<(u32, Lancamento)> = itens
            .iter()
            .map(|(chave, item)| {
                let config = item.filho("config");
                let lancamento = Lancamento {
                    executavel: item.texto("executable"),
                    argumentos: item.texto("arguments"),
                    pasta_de_trabalho: item.texto("workingdir"),
                    tipo: item.texto("type"),
                    sistemas: config.map(|c| c.texto("oslist")).unwrap_or_default(),
                    beta: config.is_some_and(|c| !c.texto("betakey").is_empty()),
                };
                (chave.parse().unwrap_or(u32::MAX), lancamento)
            })
            .collect();
        lista.sort_by_key(|(ordem, _)| *ordem);
        return Some(lista.into_iter().map(|(_, l)| l).collect());
    }
}

/// O "Jogar" da Steam neste sistema (`windows`, `linux`…): a primeira entrada padrão com executável,
/// para o sistema, fora de beta.
pub fn padrao<'a>(lancamentos: &'a [Lancamento], sistema: &str) -> Option<&'a Lancamento> {
    lancamentos.iter().find(|l| {
        let tipo_padrao = l.tipo.is_empty() || l.tipo.eq_ignore_ascii_case("default") || l.tipo.eq_ignore_ascii_case("none");
        let do_sistema = l.sistemas.trim().is_empty() || l.sistemas.split(',').any(|s| s.trim().eq_ignore_ascii_case(sistema));
        tipo_padrao && do_sistema && !l.beta && !l.executavel.trim().is_empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um `appinfo.vdf` versão 29 com um app e as entradas de `launch` dadas
    /// (`[(chave, executável, tipo, oslist)]`).
    fn arquivo_v29(app_id: u32, entradas: &[(&str, &str, &str, &str)]) -> Vec<u8> {
        fn indice(t: &str, textos: &mut Vec<String>) -> u32 {
            if let Some(i) = textos.iter().position(|x| x == t) {
                return i as u32;
            }
            textos.push(t.to_string());
            (textos.len() - 1) as u32
        }
        fn mapa(kv: &mut Vec<u8>, chave: &str, textos: &mut Vec<String>) {
            kv.push(0);
            kv.extend(indice(chave, textos).to_le_bytes());
        }
        fn texto(kv: &mut Vec<u8>, chave: &str, valor: &str, textos: &mut Vec<String>) {
            kv.push(1);
            kv.extend(indice(chave, textos).to_le_bytes());
            kv.extend(valor.as_bytes());
            kv.push(0);
        }
        let mut textos: Vec<String> = Vec::new();
        let mut kv: Vec<u8> = Vec::new();
        mapa(&mut kv, "appinfo", &mut textos);
        texto(&mut kv, "appid", &app_id.to_string(), &mut textos);
        mapa(&mut kv, "config", &mut textos);
        mapa(&mut kv, "launch", &mut textos);
        for (chave, exe, tipo, os) in entradas {
            mapa(&mut kv, chave, &mut textos);
            texto(&mut kv, "executable", exe, &mut textos);
            if !tipo.is_empty() {
                texto(&mut kv, "type", tipo, &mut textos);
            }
            mapa(&mut kv, "config", &mut textos);
            texto(&mut kv, "oslist", os, &mut textos);
            kv.extend([8, 8]);
        }
        kv.extend([8, 8, 8, 8]);

        let mut registro: Vec<u8> = vec![0; 4 + 4 + 8 + 20 + 4 + 20];
        registro.extend(&kv);
        let mut dados: Vec<u8> = Vec::new();
        dados.extend(V29.to_le_bytes());
        dados.extend(1u32.to_le_bytes());
        let posicao_tabela = dados.len();
        dados.extend(0u64.to_le_bytes());
        // Outro app antes, para conferir que ele é pulado.
        dados.extend(7u32.to_le_bytes());
        dados.extend(3u32.to_le_bytes());
        dados.extend([1, 2, 3]);
        dados.extend(app_id.to_le_bytes());
        dados.extend((registro.len() as u32).to_le_bytes());
        dados.extend(&registro);
        dados.extend(0u32.to_le_bytes());
        let tabela = dados.len() as u64;
        dados[posicao_tabela..posicao_tabela + 8].copy_from_slice(&tabela.to_le_bytes());
        dados.extend((textos.len() as u32).to_le_bytes());
        for t in &textos {
            dados.extend(t.as_bytes());
            dados.push(0);
        }
        dados
    }

    #[test]
    fn acha_o_executavel_padrao_do_sistema() {
        let dados = arquivo_v29(1856900, &[
            ("1", "KeyConfig.exe", "config", "windows"),
            ("0", "andro dunos ii.EXE", "default", "windows"),
            ("2", "jogo.sh", "default", "linux"),
        ]);
        let lista = lancamentos(&dados, 1856900).unwrap();
        assert_eq!(lista.len(), 3);
        assert_eq!(lista[0].executavel, "andro dunos ii.EXE");
        assert_eq!(padrao(&lista, "windows").unwrap().executavel, "andro dunos ii.EXE");
        assert_eq!(padrao(&lista, "linux").unwrap().executavel, "jogo.sh");
        assert!(padrao(&lista, "macos").is_none());
    }

    #[test]
    fn app_ausente_ou_arquivo_estranho_da_none() {
        let dados = arquivo_v29(10, &[("0", "a.exe", "", "")]);
        assert!(lancamentos(&dados, 11).is_none());
        assert!(lancamentos(b"\x00\x01", 10).is_none());
        assert!(lancamentos(&dados[..dados.len() / 2], 10).is_none());
        let mut outra_versao = dados.clone();
        outra_versao[0] = 0x26;
        assert!(lancamentos(&outra_versao, 10).is_none());
        // Sem tipo nem sistema: vale como padrão em qualquer sistema.
        assert_eq!(padrao(&lancamentos(&dados, 10).unwrap(), "windows").unwrap().executavel, "a.exe");
    }
}
