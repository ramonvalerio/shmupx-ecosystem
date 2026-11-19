//! Acesso à memória física que o ARM (HPS) do MiSTer compartilha com o FPGA.

use std::sync::atomic::{Ordering, fence};
use std::time::Duration;

/// Janela de memória endereçada a partir do início do espelho.
pub trait Memoria {
    fn ler_u8(&self, deslocamento: usize) -> u8;
    fn ler_u32(&self, deslocamento: usize) -> u32;
    fn escrever_u32(&mut self, deslocamento: usize, valor: u32);

    /// Garante a ordem das escritas vistas pelo FPGA (o `__sync_synchronize` do Main_MiSTer).
    fn barreira(&self) {
        fence(Ordering::SeqCst);
    }

    /// Deixa o tempo passar enquanto o FPGA trabalha.
    fn aguardar(&self, intervalo: Duration) {
        std::thread::sleep(intervalo);
    }
}

#[cfg(target_os = "linux")]
pub use devmem::DevMem;

#[cfg(target_os = "linux")]
mod devmem {
    use super::Memoria;
    use std::fs::OpenOptions;
    use std::io;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    use std::ptr;

    /// Região física mapeada por `/dev/mem` (exige root, como tudo no MiSTer).
    pub struct DevMem {
        ponteiro: *mut u8,
        tamanho: usize,
    }

    impl DevMem {
        pub fn abrir(base: u64, tamanho: usize) -> io::Result<Self> {
            let arquivo = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_SYNC)
                .open("/dev/mem")?;
            let deslocamento = libc::off_t::try_from(base)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "endereço físico fora do alcance"))?;
            // SAFETY: mapeamento novo, sem aliasing; o descritor pode ser fechado depois do mmap.
            let ponteiro = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    tamanho,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    arquivo.as_raw_fd(),
                    deslocamento,
                )
            };
            if ponteiro == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { ponteiro: ponteiro.cast(), tamanho })
        }

        fn endereco(&self, deslocamento: usize, largura: usize) -> *mut u8 {
            assert!(deslocamento + largura <= self.tamanho, "acesso fora do espelho: {deslocamento:#x}");
            assert!(deslocamento.is_multiple_of(largura), "acesso desalinhado: {deslocamento:#x}");
            // SAFETY: dentro do mapeamento, conferido acima.
            unsafe { self.ponteiro.add(deslocamento) }
        }
    }

    impl Memoria for DevMem {
        fn ler_u8(&self, deslocamento: usize) -> u8 {
            // SAFETY: endereço válido e alinhado; volatile porque o FPGA escreve por fora.
            unsafe { ptr::read_volatile(self.endereco(deslocamento, 1)) }
        }

        fn ler_u32(&self, deslocamento: usize) -> u32 {
            // SAFETY: idem, alinhado a 4.
            u32::from_le(unsafe { ptr::read_volatile(self.endereco(deslocamento, 4).cast::<u32>()) })
        }

        fn escrever_u32(&mut self, deslocamento: usize, valor: u32) {
            // SAFETY: idem, alinhado a 4.
            unsafe { ptr::write_volatile(self.endereco(deslocamento, 4).cast::<u32>(), valor.to_le()) }
        }
    }

    impl Drop for DevMem {
        fn drop(&mut self) {
            // SAFETY: desfaz exatamente o mapeamento criado em `abrir`.
            unsafe {
                libc::munmap(self.ponteiro.cast(), self.tamanho);
            }
        }
    }
}
