//! shmupx-mister: agente do ShmupX que roda no ARM do MiSTer FPGA.
//!
//! Protótipo: lê a RAM do jogo pelo espelho do core, encontra e lê o placar e, como serviço,
//! grava cada partida em disco. Ainda não envia nada ao backend.

// Fora do MiSTer só o `procurar` usa o resto do código; os testes cobrem o restante.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod config;
mod envio;
mod presenca;
mod espelho;
mod genesis;
mod memoria;
mod partida;
mod placar;
mod ranking;

use partida::Partida;
use placar::{Amostra, Perfil};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const PASTA_PADRAO: &str = "/media/fat/shmupx";

const AJUDA: &str = "\
shmupx-mister — agente ShmupX para MiSTer FPGA (protótipo)

USO
  shmupx-mister status                     core carregado e estado do espelho de RAM
  shmupx-mister dump <saida.bin>           salva os 64 KB da RAM do 68000
  shmupx-mister procurar <dump>=<placar>…  acha o endereço do placar (roda também no PC)
  shmupx-mister placar <perfil.json> [--continuo]
                                           lê o placar; --continuo mostra cada mudança
  shmupx-mister servico                    fica em segundo plano registrando as partidas
  shmupx-mister ranking <jogo> [modo] [--online]
                                           top 10 (modo padrão: hi-score), para o menu Scripts
                                           o jogador vem de `username` em <pasta>/shmupx.ini
  shmupx-mister usuarios                   lista os jogadores do shmupx.ini (id e nome)
  shmupx-mister conexao                    confere o api e o token de cada jogador (não gasta ficha)
  shmupx-mister envio-teste <id> <placar> [modo]
                                           abre, encerra e envia uma partida de Earthion
                                           (GASTA 1 FICHA: use contra um backend de teste)

OPÇÕES
  --base <endereço>   base física do espelho (padrão 0x3D000000)
  --pasta <pasta>     pasta do ShmupX (padrão /media/fat/shmupx)

Exemplo de descoberta: pause o jogo, anote o placar da tela e rode `dump`; repita com
placares diferentes e depois `procurar a.bin=12300 b.bin=45600 c.bin=78900`.";

fn main() -> ExitCode {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    match executar(&argumentos) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erro) => {
            eprintln!("erro: {erro}");
            ExitCode::FAILURE
        }
    }
}

fn executar(argumentos: &[String]) -> Result<(), String> {
    let (base, pasta, argumentos) = extrair_opcoes(argumentos)?;
    let argumentos: Vec<&str> = argumentos.iter().map(String::as_str).collect();
    match argumentos.as_slice() {
        ["status"] => hardware::status(base),
        ["dump", saida] => hardware::dump(base, Path::new(saida)),
        ["procurar", pares @ ..] if !pares.is_empty() => procurar(pares),
        ["placar", perfil] => hardware::placar(base, &carregar_perfil(perfil)?, false),
        ["placar", perfil, "--continuo"] => hardware::placar(base, &carregar_perfil(perfil)?, true),
        ["servico"] => hardware::servico(base, &pasta),
        ["servico", outra] => hardware::servico(base, Path::new(outra)),
        ["ranking", jogo] => mostrar_ranking(&pasta, jogo, partida::MODO_PADRAO, false),
        ["ranking", jogo, "--online"] => mostrar_ranking(&pasta, jogo, partida::MODO_PADRAO, true),
        ["ranking", jogo, modo] => mostrar_ranking(&pasta, jogo, modo, false),
        ["ranking", jogo, modo, "--online"] => mostrar_ranking(&pasta, jogo, modo, true),
        ["conexao"] => conexao(&pasta),
        ["envio-teste", id, placar] => envio_teste(&pasta, id, placar, "hi-score"),
        ["envio-teste", id, placar, modo] => envio_teste(&pasta, id, placar, modo),
        ["usuarios"] => {
            // Para o menu.sh montar uma pasta por jogador: "id nome" por linha, na ordem do
            // menu (último a jogar primeiro).
            for usuario in config::usuarios_em_ordem(&pasta) {
                println!("{} {}", usuario.id, usuario.nome);
            }
            Ok(())
        }
        [] | ["-h" | "--help" | "ajuda"] => {
            println!("{AJUDA}");
            Ok(())
        }
        _ => Err(format!("comando não reconhecido\n\n{AJUDA}")),
    }
}

fn extrair_opcoes(argumentos: &[String]) -> Result<(u64, PathBuf, Vec<String>), String> {
    let mut base = espelho::BASE_PADRAO;
    let mut pasta = PathBuf::from(PASTA_PADRAO);
    let mut resto = Vec::new();
    let mut itens = argumentos.iter();
    while let Some(item) = itens.next() {
        match item.as_str() {
            "--base" => {
                let valor = itens.next().ok_or("--base precisa de um endereço")?;
                base = u64::from_str_radix(valor.trim_start_matches("0x"), 16)
                    .map_err(|_| format!("endereço inválido: {valor}"))?;
            }
            "--pasta" => pasta = PathBuf::from(itens.next().ok_or("--pasta precisa de um caminho")?),
            _ => resto.push(item.clone()),
        }
    }
    Ok((base, pasta, resto))
}

fn conexao(pasta: &Path) -> Result<(), String> {
    let base = config::api(pasta).ok_or("sem api no shmupx.ini: as partidas ficam só no ranking local")?;
    println!("api: {base}");
    for usuario in config::usuarios(pasta) {
        let situacao = match config::token(pasta, &usuario) {
            None => "sem token (só ranking local)".to_owned(),
            Some(token) => match (envio::Api { base: base.clone(), token }).verificar() {
                Ok(()) => "conectado".to_owned(),
                Err(motivo) => format!("ERRO: {motivo}"),
            },
        };
        println!("{:<12} {situacao}", usuario.nome);
    }
    Ok(())
}

fn envio_teste(pasta: &Path, id: &str, placar: &str, modo: &str) -> Result<(), String> {
    let placar: u64 = placar.parse().map_err(|_| format!("placar inválido: {placar}"))?;
    let usuario = config::identificar(pasta, &format!("SX_{id}_hs")).0;
    let perfil = Perfil::de_json(
        &std::fs::read_to_string(pasta.join("perfis").join("earthion.json")).map_err(|e| format!("perfil: {e}"))?,
    )?;
    let envio = envio::preparar(pasta, &usuario, &perfil, modo)?;
    let run_id = envio.api.iniciar(&envio.jogo_id, envio.modo, envio.versao.as_deref())?;
    println!("partida aberta: {run_id}");
    println!("envio: {}", envio::finalizar(pasta, &envio, &run_id, placar));
    Ok(())
}

fn mostrar_ranking(pasta: &Path, jogo: &str, modo: &str, online: bool) -> Result<(), String> {
    let titulo = std::fs::read_to_string(pasta.join("perfis").join(format!("{jogo}.json")))
        .ok()
        .and_then(|texto| Perfil::de_json(&texto).ok())
        .map_or_else(|| jogo.to_owned(), |perfil| perfil.titulo().to_owned());
    if online {
        print!("{}", ranking::tela_online(&titulo, modo));
    } else {
        let partidas = Partida::carregar_todas(&pasta.join("partidas"));
        let linhas = ranking::top(&partidas, jogo, modo, &config::jogador_principal(pasta), ranking::TOP);
        print!("{}", ranking::tela_local(&titulo, modo, &linhas));
    }
    Ok(())
}

fn carregar_perfil(caminho: &str) -> Result<Perfil, String> {
    let texto = std::fs::read_to_string(caminho).map_err(|e| format!("não consegui ler {caminho}: {e}"))?;
    Perfil::de_json(&texto)
}

fn procurar(pares: &[&str]) -> Result<(), String> {
    let mut amostras = Vec::new();
    for par in pares {
        let (caminho, placar) = par.rsplit_once('=').ok_or_else(|| format!("use <dump>=<placar>: {par}"))?;
        let placar: u64 = placar.parse().map_err(|_| format!("placar inválido: {placar}"))?;
        let wram = std::fs::read(caminho).map_err(|e| format!("não consegui ler {caminho}: {e}"))?;
        if wram.len() != genesis::TAMANHO_WRAM {
            return Err(format!("{caminho} tem {} bytes; um dump tem 65536", wram.len()));
        }
        amostras.push(Amostra { wram, placar });
    }
    if amostras.len() < 2 {
        eprintln!("aviso: com um dump só aparecem muitos candidatos; use 3 ou mais placares diferentes");
    }

    let candidatos = placar::procurar(&amostras);
    match candidatos.len() {
        0 => println!("nenhum candidato. Confira se o placar anotado é o da tela no momento de cada dump."),
        1 => println!("encontrado:"),
        n => println!("{n} candidatos (mais dumps com placares diferentes reduzem a lista):"),
    }
    for campo in candidatos.iter().take(50) {
        println!("  {campo}");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod hardware {
    use crate::espelho::{self, Espelho};
    use crate::genesis;
    use crate::memoria::DevMem;
    use crate::config;
    use crate::partida::Partida;
    use crate::placar::{self, CampoPlacar, Estabilizador, Perfil};
    use std::path::Path;
    use std::time::Duration;

    fn abrir(base: u64) -> Result<Espelho<DevMem>, String> {
        let memoria = DevMem::abrir(base, espelho::TAMANHO_MAPA)
            .map_err(|e| format!("não consegui mapear /dev/mem em {base:#x}: {e} (precisa rodar como root)"))?;
        Ok(Espelho::novo(memoria))
    }

    fn core_atual() -> String {
        std::fs::read_to_string("/tmp/CORENAME").map(|n| n.trim().to_owned()).unwrap_or_else(|_| "?".into())
    }

    pub fn status(base: u64) -> Result<(), String> {
        println!("core carregado: {}", core_atual());
        match abrir(base)?.cabecalho() {
            None => println!("espelho de RAM: ausente (core sem suporte a RetroAchievements)"),
            Some(c) => println!(
                "espelho de RAM: ativo — versão do core {:#06x}, protocolo {:#04x}, quadro {}, regiões {}",
                c.versao_core, c.versao_protocolo, c.quadro, c.regioes
            ),
        }
        Ok(())
    }

    pub fn dump(base: u64, saida: &Path) -> Result<(), String> {
        let mut espelho = abrir(base)?;
        let wram = genesis::despejar_wram(&mut espelho).map_err(|e| e.to_string())?;
        std::fs::write(saida, &wram).map_err(|e| format!("não consegui gravar {}: {e}", saida.display()))?;
        println!("{} bytes gravados em {}", wram.len(), saida.display());
        Ok(())
    }

    pub fn placar(base: u64, perfil: &Perfil, continuo: bool) -> Result<(), String> {
        let campo = perfil.campo()?;
        let mut espelho = abrir(base)?;
        let (enderecos, ordem) = genesis::enderecos_ordenados(campo.deslocamento, campo.bytes);
        let id = espelho.publicar(&enderecos).map_err(|e| e.to_string())?;

        let mut ultimo_quadro = None;
        let mut estabilizador = Estabilizador::default();
        loop {
            let leitura = espelho.ler(id, enderecos.len(), ultimo_quadro).map_err(|e| e.to_string())?;
            ultimo_quadro = Some(leitura.quadro);
            let bytes: Vec<u8> = ordem.iter().map(|&i| leitura.valores[i]).collect();
            if let Some(placar) = estabilizador.observar(campo.decodificar(&bytes)) {
                println!("quadro {} placar {placar}", leitura.quadro);
                if !continuo {
                    return Ok(());
                }
            }
        }
    }

    /// Estado para a tela do menu Scripts. Fica em /tmp, que está na RAM: não gasta o SD.
    /// As mensagens são só ASCII: o console do MiSTer pode não ter acentos e não há `iconv`.
    const ARQUIVO_ESTADO: &str = "/tmp/shmupx-estado.txt";

    fn anunciar(estado: &str, ultimo: &mut String) {
        if estado != ultimo {
            let _ = std::fs::write(ARQUIVO_ESTADO, format!("{estado}\n"));
            eprintln!("{estado}");
            estado.clone_into(ultimo);
        }
    }

    /// Pelo arquivo do core (`/tmp/RBFNAME`), não pelo CORENAME: no menu ShmupX o CORENAME é
    /// `SX_<jogador>_<modo>` (o MiSTer corta em 31 caracteres, não cabe o nome do core).
    fn core_de_mega_drive() -> bool {
        let rbf = std::fs::read_to_string("/tmp/RBFNAME").unwrap_or_default().to_ascii_lowercase();
        rbf.contains("megadrive") || rbf.contains("genesis")
    }

    /// Espera um core de Mega Drive com espelho, lê o placar enquanto o jogo roda e grava a
    /// partida em `<pasta>/partidas/` quando o core para (volta ao menu ou troca de core).
    pub fn servico(base: u64, pasta: &Path) -> Result<(), String> {
        // Jogo aberto na conta (aberto/fechado ao backend): vale para todo jogo do menu ShmupX,
        // inclusive os de arcade sem leitura de placar, por isso roda antes do espelho.
        let vigia = pasta.to_path_buf();
        std::thread::spawn(move || crate::presenca::vigiar(vigia));
        let mut espelho = abrir(base)?;
        let mut ultimo_estado = String::new();
        let mut core_anterior = String::new();
        let mut ultimo_reenvio = std::time::Instant::now();
        loop {
            let core = core_atual();
            // Qualquer jogo aberto pelo menu ShmupX (até os sem leitura de placar, como os de
            // arcade) leva o jogador para o topo do menu.
            if core != core_anterior {
                if config::core_do_shmupx(&core) {
                    subir_no_menu(pasta, &config::identificar(pasta, &core).0);
                }
                core_anterior.clone_from(&core);
            }
            if !core_de_mega_drive() || !core_vivo(&espelho) {
                anunciar("Aguardando um jogo ShmupX", &mut ultimo_estado);
                // Ocioso: tenta de novo, a cada minuto, as partidas que ficaram sem envio.
                if ultimo_reenvio.elapsed() >= Duration::from_secs(60) {
                    let enviadas = crate::envio::reenviar_pendentes(pasta);
                    if enviadas > 0 {
                        eprintln!("{enviadas} partida(s) pendente(s) enviada(s)");
                    }
                    ultimo_reenvio = std::time::Instant::now();
                }
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let (prontos, pendentes) = placar::perfis_do_sistema(&pasta.join("perfis"), "megadrive");
            // Por enquanto o jogo não é identificado: vale o primeiro perfil pronto.
            let Some((perfil, campo)) = prontos.into_iter().next() else {
                let estado = match pendentes.first() {
                    Some(jogo) => format!("{jogo}: endereco do placar ainda nao descoberto (veja o README)"),
                    None => "Nenhum perfil de Mega Drive em perfis/".to_owned(),
                };
                anunciar(&estado, &mut ultimo_estado);
                std::thread::sleep(Duration::from_secs(2));
                continue;
            };
            anunciar(&format!("Jogando {}", perfil.jogo), &mut ultimo_estado);
            let (usuario, modo) = config::identificar(pasta, &core);
            let modo = modo.unwrap_or_else(|| perfil.modo_principal());
            let mut nova = Partida::nova(&perfil.jogo, modo, &usuario.nome, &core);
            let envio = match crate::envio::preparar(pasta, &usuario, &perfil, modo) {
                Ok(envio) => Some(envio),
                Err(motivo) => {
                    nova.envio = Some(format!("local: {motivo}"));
                    None
                }
            };
            let mut partida = acompanhar(&mut espelho, nova, campo, envio.as_ref(), &mut ultimo_estado);
            if let (Some(envio), Some(run_id)) = (envio.as_ref(), partida.run_id.clone()) {
                anunciar(&format!("Enviando {} ao ShmupX...", partida.jogo), &mut ultimo_estado);
                let placar = partida.placar_maximo.unwrap_or(0);
                partida.envio = Some(crate::envio::finalizar(pasta, envio, &run_id, placar));
            }
            if partida.tem_placar() || partida.run_id.is_some() {
                if let Err(e) = partida.salvar(&pasta.join("partidas")) {
                    eprintln!("não consegui salvar a partida: {e}");
                }
                let pontos = partida.placar_maximo.unwrap_or(0);
                anunciar(&format!("Ultima partida: {} - {pontos} pontos", partida.jogo), &mut ultimo_estado);
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }

    /// Quem começou a jogar vai para o topo do menu ShmupX: na volta ao menu, a pasta dele
    /// já é a primeira. O menu.sh refaz as pastas (só nomes de pasta e .mgl pequenos).
    fn subir_no_menu(pasta: &Path, usuario: &config::Usuario) {
        if let Err(e) = config::registrar_ultimo(pasta, usuario) {
            eprintln!("não consegui gravar o último jogador: {e}");
            return;
        }
        match std::process::Command::new(pasta.join("menu.sh")).status() {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("menu.sh terminou com {s}"),
            Err(e) => eprintln!("não consegui rodar o menu.sh: {e}"),
        }
    }

    /// O magic continua na DDR3 depois que o core sai; só vale se o contador de quadros andar.
    fn core_vivo(espelho: &Espelho<DevMem>) -> bool {
        let Some(antes) = espelho.cabecalho() else { return false };
        std::thread::sleep(Duration::from_millis(100));
        espelho.cabecalho().is_some_and(|depois| depois.quadro != antes.quadro)
    }

    fn acompanhar(
        espelho: &mut Espelho<DevMem>,
        mut partida: Partida,
        campo: CampoPlacar,
        envio: Option<&crate::envio::Envio>,
        ultimo_estado: &mut String,
    ) -> Partida {
        let (jogo, core) = (partida.jogo.clone(), partida.core.clone());
        let (enderecos, ordem) = genesis::enderecos_ordenados(campo.deslocamento, campo.bytes);
        let Ok(id) = espelho.publicar(&enderecos) else {
            return partida;
        };
        let mut ultimo_quadro = None;
        let mut estabilizador = Estabilizador::default();
        let limite = crate::partida::limite_em_quadros(&partida.modo);
        let mut primeiro_ponto: Option<u32> = None;
        let mut tempo_esgotado = false;
        // Abertura da partida no servidor (gasta a ficha), numa thread: o curl pode levar
        // segundos e a leitura do placar não pode parar.
        let mut abertura: Option<std::thread::JoinHandle<Result<String, String>>> = None;
        // Termina quando o FPGA para de responder (core descarregado) ou o core muda.
        while let Ok(leitura) = espelho.ler(id, enderecos.len(), ultimo_quadro) {
            ultimo_quadro = Some(leitura.quadro);
            partida.quadros += 1;
            // Caravan: só vale o placar feito até o fim do tempo, contado a partir do primeiro
            // ponto (o serviço não distingue a tela-título do início da partida).
            if let (Some(limite), Some(inicio)) = (limite, primeiro_ponto)
                && !tempo_esgotado
                && leitura.quadro.wrapping_sub(inicio) >= limite
            {
                tempo_esgotado = true;
                let pontos = partida.placar_maximo.unwrap_or(0);
                anunciar(&format!("{jogo}: tempo esgotado - {pontos} pontos"), ultimo_estado);
            }
            let bytes: Vec<u8> = ordem.iter().map(|&i| leitura.valores[i]).collect();
            if let Some(placar) = estabilizador.observar(campo.decodificar(&bytes))
                && !tempo_esgotado
            {
                if placar > 0 && primeiro_ponto.is_none() {
                    primeiro_ponto = Some(leitura.quadro);
                    if let Some(envio) = envio.cloned() {
                        abertura = Some(std::thread::spawn(move || {
                            envio.api.iniciar(&envio.jogo_id, envio.modo, envio.versao.as_deref())
                        }));
                    }
                }
                partida.registrar(placar);
                anunciar(&format!("Jogando {jogo} - placar {placar}"), ultimo_estado);
            }
            if abertura.as_ref().is_some_and(|a| a.is_finished()) {
                aplicar_abertura(&mut partida, abertura.take());
            }
            if partida.quadros.is_multiple_of(60) && core_atual() != core {
                break;
            }
        }
        aplicar_abertura(&mut partida, abertura);
        partida.encerrar();
        partida
    }

    /// Resultado do `/api/runs/start`: o `run_id`, ou o motivo de a partida ficar só local
    /// (sem fichas, sem internet, token revogado...).
    fn aplicar_abertura(partida: &mut Partida, abertura: Option<std::thread::JoinHandle<Result<String, String>>>) {
        let Some(abertura) = abertura else { return };
        match abertura.join() {
            Ok(Ok(run_id)) => partida.run_id = Some(run_id),
            Ok(Err(motivo)) => partida.envio = Some(format!("local: {motivo}")),
            Err(_) => partida.envio = Some("local: falha interna ao abrir a partida".to_owned()),
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod hardware {
    use crate::placar::Perfil;
    use std::path::Path;

    const SO_MISTER: &str = "este comando lê a memória do MiSTer e só roda nele (Linux ARM)";

    pub fn status(_: u64) -> Result<(), String> {
        Err(SO_MISTER.into())
    }

    pub fn dump(_: u64, _: &Path) -> Result<(), String> {
        Err(SO_MISTER.into())
    }

    pub fn placar(_: u64, perfil: &Perfil, _: bool) -> Result<(), String> {
        perfil.campo()?;
        Err(SO_MISTER.into())
    }

    pub fn servico(_: u64, _: &Path) -> Result<(), String> {
        Err(SO_MISTER.into())
    }
}
