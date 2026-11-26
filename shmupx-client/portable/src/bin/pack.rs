//! Empacotador do ShmupX Portable (usado por `scripts/make-portable.ps1`, não vai ao jogador):
//! compacta a pasta do Client e anexa ao lançador.
//!
//! `shmupx-portable-pack <pasta do Client> <lançador .exe> <versão> <saída .exe>`

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use shmupx_portable::{Versao, rodape};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pasta, lancador, versao, saida] = args.as_slice() else {
        eprintln!("uso: shmupx-portable-pack <pasta do Client> <lançador .exe> <versão> <saída .exe>");
        std::process::exit(2);
    };
    let Some(versao) = Versao::ler(versao) else {
        eprintln!("versão inválida: {versao} (use 1.2.3)");
        std::process::exit(2);
    };
    if let Err(e) = empacotar(Path::new(pasta), Path::new(lancador), versao, Path::new(saida)) {
        eprintln!("falha ao gerar o ShmupX Portable: {e}");
        std::process::exit(1);
    }
}

/// Arquivos da pasta, em ordem, com o caminho relativo (separado por `/`, como o zip espera).
fn arquivos(raiz: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut lista = Vec::new();
    let mut pendentes = vec![raiz.to_path_buf()];
    while let Some(pasta) = pendentes.pop() {
        for entrada in fs::read_dir(&pasta)? {
            let caminho = entrada?.path();
            if caminho.is_dir() {
                pendentes.push(caminho);
            } else {
                let relativo = caminho.strip_prefix(raiz).expect("dentro da raiz");
                let nome = relativo.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                lista.push((nome, caminho));
            }
        }
    }
    lista.sort();
    Ok(lista)
}

fn empacotar(pasta: &Path, lancador: &Path, versao: Versao, saida: &Path) -> io::Result<()> {
    if !pasta.join("shmupx-client.exe").is_file() {
        return Err(io::Error::other(format!("{} não tem shmupx-client.exe", pasta.display())));
    }
    let zip_temporario = saida.with_extension("zip.tmp");
    let lista = arquivos(pasta)?;
    {
        let mut zip = ZipWriter::new(BufWriter::new(File::create(&zip_temporario)?));
        for (nome, caminho) in &lista {
            let tamanho = fs::metadata(caminho)?.len();
            let opcoes = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .large_file(tamanho >= u32::MAX as u64);
            zip.start_file(nome.as_str(), opcoes).map_err(io::Error::other)?;
            io::copy(&mut File::open(caminho)?, &mut zip)?;
        }
        zip.finish().map_err(io::Error::other)?.flush()?;
    }
    let tamanho_zip = fs::metadata(&zip_temporario)?.len();
    let mut final_ = BufWriter::new(File::create(saida)?);
    io::copy(&mut File::open(lancador)?, &mut final_)?;
    io::copy(&mut File::open(&zip_temporario)?, &mut final_)?;
    final_.write_all(&rodape(versao, tamanho_zip))?;
    final_.flush()?;
    fs::remove_file(&zip_temporario)?;
    println!(
        "OK: {} ({} arquivos, {} MB, versão {versao})",
        saida.display(),
        lista.len(),
        fs::metadata(saida)?.len() / 1024 / 1024
    );
    Ok(())
}
