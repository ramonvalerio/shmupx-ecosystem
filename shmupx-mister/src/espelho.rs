//! Protocolo do espelho de RAM dos cores com suporte a RetroAchievements (fork de odelot).
//!
//! O core modificado escreve na DDR3, a partir de `0x3D000000`, um cabeçalho com o magic
//! `RACH`. No Mega Drive a RAM não é copiada inteira: o ARM publica uma lista ordenada de
//! até 4096 endereços e, a cada VBlank, o FPGA devolve um byte por endereço.
//!
//! Layout (deslocamentos a partir da base):
//! - `0x00000` cabeçalho: magic u32, regiões u8, flags u8 (bit 0 = varrendo), versão do core
//!   u16, contador de quadros u32; byte `0x17` = versão do protocolo.
//! - `0x40000` pedido: quantidade u32, id u32 (escrito por último, é o "commit"); endereços
//!   u32 a partir de `0x40008`.
//! - `0x48000` resposta: id u32, quadro u32; valores (1 byte por endereço, na ordem da lista)
//!   a partir de `0x48008`. O FPGA escreve os valores antes do cabeçalho da resposta.
//!
//! Este módulo reimplementa o protocolo a partir da descrição; não contém código do Main_MiSTer.

use crate::memoria::Memoria;
use std::fmt;
use std::time::{Duration, Instant};

pub const BASE_PADRAO: u64 = 0x3D00_0000;
pub const TAMANHO_MAPA: usize = 0x8_0000;
pub const MAGIC: u32 = 0x5241_4348;
pub const MAX_ENDERECOS: usize = 4096;

const FLAG_VARRENDO: u8 = 0x01;
const BYTE_VERSAO_PROTOCOLO: usize = 0x17;
pub(crate) const PEDIDO_BASE: usize = 0x4_0000;
pub(crate) const RESPOSTA_BASE: usize = 0x4_8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cabecalho {
    pub regioes: u8,
    pub flags: u8,
    pub versao_core: u16,
    pub quadro: u32,
    pub versao_protocolo: u8,
}

impl Cabecalho {
    pub fn varrendo(&self) -> bool {
        self.flags & FLAG_VARRENDO != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leitura {
    /// Quadro do FPGA em que os valores foram lidos.
    pub quadro: u32,
    /// Um byte por endereço pedido, na mesma ordem.
    pub valores: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Erro {
    /// Sem o magic `RACH`: core sem espelho (core oficial, outro sistema ou nada carregado).
    SemEspelho,
    ListaInvalida(&'static str),
    TempoEsgotado(&'static str),
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erro::SemEspelho => write!(
                f,
                "espelho de RAM não encontrado: carregue o core MegaDrive com suporte a RetroAchievements"
            ),
            Erro::ListaInvalida(motivo) => write!(f, "lista de endereços inválida: {motivo}"),
            Erro::TempoEsgotado(etapa) => write!(f, "o FPGA não respondeu a tempo ({etapa})"),
        }
    }
}

impl std::error::Error for Erro {}

pub struct Espelho<M: Memoria> {
    memoria: M,
    ultimo_id: u32,
    intervalo: Duration,
    limite: Duration,
}

impl<M: Memoria> Espelho<M> {
    pub fn novo(memoria: M) -> Self {
        // Começa depois do último id respondido, para nunca confundir a resposta de uma
        // execução anterior com a nossa.
        let ultimo_id = memoria.ler_u32(RESPOSTA_BASE);
        Self { memoria, ultimo_id, intervalo: Duration::from_millis(1), limite: Duration::from_secs(1) }
    }

    pub fn cabecalho(&self) -> Option<Cabecalho> {
        if self.memoria.ler_u32(0) != MAGIC {
            return None;
        }
        let palavra = self.memoria.ler_u32(4);
        Some(Cabecalho {
            regioes: palavra as u8,
            flags: (palavra >> 8) as u8,
            versao_core: (palavra >> 16) as u16,
            quadro: self.memoria.ler_u32(8),
            versao_protocolo: self.memoria.ler_u8(BYTE_VERSAO_PROTOCOLO),
        })
    }

    /// Publica a lista de endereços (no formato do RetroAchievements) e devolve o id do pedido.
    pub fn publicar(&mut self, enderecos: &[u32]) -> Result<u32, Erro> {
        if enderecos.is_empty() {
            return Err(Erro::ListaInvalida("vazia"));
        }
        if enderecos.len() > MAX_ENDERECOS {
            return Err(Erro::ListaInvalida("mais de 4096 endereços"));
        }
        if enderecos.windows(2).any(|par| par[0] >= par[1]) {
            return Err(Erro::ListaInvalida("precisa estar em ordem crescente e sem repetição"));
        }
        self.cabecalho().ok_or(Erro::SemEspelho)?;
        // Como o Main_MiSTer: não troca a lista no meio de uma varredura.
        self.aguardar("fim da varredura", |e| e.cabecalho().is_some_and(|c| !c.varrendo()))?;

        for (indice, endereco) in enderecos.iter().enumerate() {
            self.memoria.escrever_u32(PEDIDO_BASE + 8 + indice * 4, *endereco);
        }
        self.memoria.barreira();
        self.memoria.escrever_u32(PEDIDO_BASE, enderecos.len() as u32);
        self.memoria.barreira();
        let id = match self.ultimo_id.wrapping_add(1) {
            0 => 1,
            id => id,
        };
        self.memoria.escrever_u32(PEDIDO_BASE + 4, id);
        self.memoria.barreira();
        self.ultimo_id = id;
        Ok(id)
    }

    /// Lê a resposta do pedido `id`, num quadro posterior a `apos_quadro` (se informado).
    ///
    /// A leitura é consistente: o FPGA não estava varrendo antes nem depois da cópia, e o
    /// quadro da resposta não mudou durante ela.
    pub fn ler(&self, id: u32, quantidade: usize, apos_quadro: Option<u32>) -> Result<Leitura, Erro> {
        let inicio = Instant::now();
        loop {
            if let Some(leitura) = self.tentar_ler(id, quantidade, apos_quadro)? {
                return Ok(leitura);
            }
            if inicio.elapsed() > self.limite {
                return Err(Erro::TempoEsgotado("resposta do pedido"));
            }
            self.memoria.aguardar(self.intervalo);
        }
    }

    /// Publica e lê uma vez.
    pub fn consultar(&mut self, enderecos: &[u32]) -> Result<Leitura, Erro> {
        let id = self.publicar(enderecos)?;
        self.ler(id, enderecos.len(), None)
    }

    fn tentar_ler(&self, id: u32, quantidade: usize, apos_quadro: Option<u32>) -> Result<Option<Leitura>, Erro> {
        let antes = self.cabecalho().ok_or(Erro::SemEspelho)?;
        let id_resposta = self.memoria.ler_u32(RESPOSTA_BASE);
        let quadro = self.memoria.ler_u32(RESPOSTA_BASE + 4);
        let novo = apos_quadro.is_none_or(|q| (quadro.wrapping_sub(q) as i32) > 0);
        if antes.varrendo() || id_resposta != id || !novo {
            return Ok(None);
        }
        self.memoria.barreira();
        let valores = (0..quantidade).map(|i| self.memoria.ler_u8(RESPOSTA_BASE + 8 + i)).collect();
        self.memoria.barreira();
        let depois = self.cabecalho().ok_or(Erro::SemEspelho)?;
        let estavel = !depois.varrendo()
            && self.memoria.ler_u32(RESPOSTA_BASE) == id
            && self.memoria.ler_u32(RESPOSTA_BASE + 4) == quadro;
        Ok(estavel.then_some(Leitura { quadro, valores }))
    }

    fn aguardar(&self, etapa: &'static str, pronto: impl Fn(&Self) -> bool) -> Result<(), Erro> {
        let inicio = Instant::now();
        while !pronto(self) {
            if inicio.elapsed() > self.limite {
                return Err(Erro::TempoEsgotado(etapa));
            }
            self.memoria.aguardar(self.intervalo);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod simulador {
    //! FPGA de mentira: cada espera do agente conta como um VBlank com varredura.

    use super::*;
    use std::cell::{Cell, RefCell};

    pub struct FpgaSimulado {
        pub ddr: RefCell<Vec<u8>>,
        /// RAM de trabalho do 68000, na ordem do próprio 68000 (big-endian).
        pub wram: RefCell<Vec<u8>>,
        quadro: Cell<u32>,
        pub ativo: bool,
    }

    impl FpgaSimulado {
        pub fn novo(wram: Vec<u8>) -> Self {
            let fpga = Self {
                ddr: RefCell::new(vec![0; TAMANHO_MAPA]),
                wram: RefCell::new(wram),
                quadro: Cell::new(0),
                ativo: true,
            };
            fpga.escrever(0, MAGIC);
            fpga.escrever(4, 0x0200 << 16);
            fpga.ddr.borrow_mut()[BYTE_VERSAO_PROTOCOLO] = 0x05;
            fpga
        }

        fn ler(&self, deslocamento: usize) -> u32 {
            let ddr = self.ddr.borrow();
            u32::from_le_bytes(ddr[deslocamento..deslocamento + 4].try_into().unwrap())
        }

        fn escrever(&self, deslocamento: usize, valor: u32) {
            self.ddr.borrow_mut()[deslocamento..deslocamento + 4].copy_from_slice(&valor.to_le_bytes());
        }

        fn varrer(&self) {
            if !self.ativo {
                return;
            }
            let quantidade = (self.ler(PEDIDO_BASE) as usize).min(MAX_ENDERECOS);
            let id = self.ler(PEDIDO_BASE + 4);
            for i in 0..quantidade {
                let endereco = self.ler(PEDIDO_BASE + 8 + i * 4) as usize;
                // Convenção RA: endereço par = byte baixo da palavra = byte ímpar no 68000.
                let valor = self.wram.borrow().get(endereco ^ 1).copied().unwrap_or(0);
                self.ddr.borrow_mut()[RESPOSTA_BASE + 8 + i] = valor;
            }
            let quadro = self.quadro.get() + 1;
            self.quadro.set(quadro);
            self.escrever(8, quadro);
            self.escrever(RESPOSTA_BASE, id);
            self.escrever(RESPOSTA_BASE + 4, quadro);
        }
    }

    impl Memoria for FpgaSimulado {
        fn ler_u8(&self, deslocamento: usize) -> u8 {
            self.ddr.borrow()[deslocamento]
        }

        fn ler_u32(&self, deslocamento: usize) -> u32 {
            self.ler(deslocamento)
        }

        fn escrever_u32(&mut self, deslocamento: usize, valor: u32) {
            self.escrever(deslocamento, valor);
        }

        fn aguardar(&self, _: Duration) {
            self.varrer();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::simulador::FpgaSimulado;
    use super::*;

    #[test]
    fn le_os_bytes_pedidos_na_convencao_do_retroachievements() {
        let mut wram = vec![0; 0x10000];
        wram[0x1000] = 0x12; // byte alto da palavra em 0x1000 (68000)
        wram[0x1001] = 0x34;
        let mut espelho = Espelho::novo(FpgaSimulado::novo(wram));

        let leitura = espelho.consultar(&[0x1000, 0x1001]).unwrap();

        assert_eq!(leitura.valores, vec![0x34, 0x12]);
    }

    #[test]
    fn ids_continuam_depois_da_ultima_resposta() {
        // Uma execução anterior deixou o pedido 41 respondido.
        let fpga = FpgaSimulado::novo(vec![0; 0x10000]);
        fpga.ddr.borrow_mut()[RESPOSTA_BASE..RESPOSTA_BASE + 4].copy_from_slice(&41u32.to_le_bytes());
        let mut espelho = Espelho::novo(fpga);

        assert_eq!(espelho.publicar(&[0]).unwrap(), 42);
        assert_eq!(espelho.publicar(&[0]).unwrap(), 43);
    }

    #[test]
    fn recusa_lista_fora_de_ordem_ou_grande_demais() {
        let mut espelho = Espelho::novo(FpgaSimulado::novo(vec![0; 0x10000]));

        assert!(matches!(espelho.publicar(&[2, 1]), Err(Erro::ListaInvalida(_))));
        assert!(matches!(espelho.publicar(&[1, 1]), Err(Erro::ListaInvalida(_))));
        let grande: Vec<u32> = (0..=MAX_ENDERECOS as u32).collect();
        assert!(matches!(espelho.publicar(&grande), Err(Erro::ListaInvalida(_))));
    }

    #[test]
    fn sem_magic_nao_ha_espelho() {
        let fpga = FpgaSimulado::novo(vec![0; 0x10000]);
        fpga.ddr.borrow_mut()[0..4].fill(0);
        let mut espelho = Espelho::novo(fpga);

        assert!(espelho.cabecalho().is_none());
        assert_eq!(espelho.publicar(&[0]), Err(Erro::SemEspelho));
    }

    #[test]
    fn leitura_seguinte_vem_de_um_quadro_novo() {
        let mut espelho = Espelho::novo(FpgaSimulado::novo(vec![0; 0x10000]));
        let id = espelho.publicar(&[0]).unwrap();
        let primeira = espelho.ler(id, 1, None).unwrap();

        let segunda = espelho.ler(id, 1, Some(primeira.quadro)).unwrap();

        assert!(segunda.quadro > primeira.quadro);
    }

    #[test]
    fn sem_resposta_do_fpga_esgota_o_tempo() {
        let mut fpga = FpgaSimulado::novo(vec![0; 0x10000]);
        fpga.ativo = false;
        let mut espelho = Espelho::novo(fpga);
        espelho.limite = Duration::from_millis(20);

        assert!(matches!(espelho.consultar(&[0]), Err(Erro::TempoEsgotado(_))));
    }
}
