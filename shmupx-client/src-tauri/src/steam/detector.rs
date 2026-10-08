/// Jogo Steam detectado em execução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedGame {
    pub app_id: u32,
    pub name: String,
    pub pid: u32,
}

/// Porta do domínio: quem sabe dizer qual jogo está ativo.
/// A implementação concreta (varredura de processos) vem em TASKS.md.
pub trait GameDetector: Send + Sync {
    fn current_game(&self) -> Option<DetectedGame>;
}
