// Edge Function `robotx-voice` (Deno/Supabase).
//
// Pipeline: recebe um .wav do Client -> transcreve (Whisper) -> gera embedding da pergunta ->
// busca vetorial isolada por jogo via RPC `buscar_conhecimento_robotx` -> monta o prompt com o
// contexto recuperado -> chat completion -> sintetiza a resposta em áudio (TTS) -> devolve o
// áudio pronto para o Client reproduzir.
//
// Ver docs/adr/003-agente-robotx-e-salas.md: o isolamento de RAG por jogo é a regra de segurança
// central desta função — nunca é opcional, nunca depende do cliente ter se comportado bem.

import { createClient } from "jsr:@supabase/supabase-js@2";
import OpenAI from "npm:openai@4";

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, x-client-info, apikey, content-type",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

const SYSTEM_PROMPT = `
Você é o RobotX, o tutor de arcade da plataforma ShmupX: amigável, direto, com o tom de quem
já viu muitas runs e gosta de ajudar o jogador a melhorar.

Responda SOMENTE com base nos trechos de contexto fornecidos pelo usuário sobre o jogo da sala
atual. Nunca misture informações de outros jogos nem invente estratégias que não estejam no
contexto. Se o contexto não tiver a resposta, diga claramente que não sabe — não tente adivinhar.
Responda em português, em 2 ou 3 frases curtas, como se estivesse falando ao vivo pelo microfone.
`.trim();

const FALLBACK_NO_CONTEXT =
  "Não encontrei nada sobre isso no meu material desse jogo. Tenta perguntar de outro jeito?";
const FALLBACK_TECHNICAL_ERROR =
  "Tive um problema técnico aqui do meu lado. Pode repetir a pergunta?";
const FALLBACK_EMPTY_TRANSCRIPT =
  "Não consegui entender o que você disse. Pode repetir, bem perto do microfone?";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isUuid(value: FormDataEntryValue | null): value is string {
  return typeof value === "string" && UUID_PATTERN.test(value);
}

function jsonError(
  status: number,
  message: string,
  extra: Record<string, unknown> = {},
): Response {
  return new Response(JSON.stringify({ error: message, ...extra }), {
    status,
    headers: { ...CORS_HEADERS, "Content-Type": "application/json" },
  });
}

/**
 * Sintetiza `message` em áudio e devolve como resposta HTTP pronta para o Client reproduzir.
 * Se o próprio TTS falhar (ex.: OpenAI inteiro fora do ar), cai para uma resposta só em texto
 * em vez de travar o Client esperando um áudio que nunca vai chegar.
 */
async function respondWithFallback(
  openai: OpenAI,
  message: string,
  status: number,
): Promise<Response> {
  try {
    const speech = await openai.audio.speech.create({
      model: "tts-1",
      voice: "alloy",
      input: message,
    });
    const audioBuffer = await speech.arrayBuffer();

    return new Response(audioBuffer, {
      status,
      headers: {
        ...CORS_HEADERS,
        "Content-Type": "audio/mpeg",
        "X-Robotx-Answer": encodeURIComponent(message),
      },
    });
  } catch (err) {
    console.error("robotx-voice: TTS do fallback também falhou", err);
    return jsonError(502, "falha ao gerar áudio de resposta", { fallback_text: message });
  }
}

Deno.serve(async (req: Request) => {
  if (req.method === "OPTIONS") {
    return new Response("ok", { headers: CORS_HEADERS });
  }

  if (req.method !== "POST") {
    return jsonError(405, "método não suportado; use POST multipart/form-data");
  }

  const supabaseUrl = Deno.env.get("SUPABASE_URL");
  const serviceRoleKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  const openaiApiKey = Deno.env.get("OPENAI_API_KEY");

  if (!supabaseUrl || !serviceRoleKey || !openaiApiKey) {
    console.error("robotx-voice: variáveis de ambiente obrigatórias ausentes no projeto");
    return jsonError(500, "função mal configurada no servidor");
  }

  // Service role: a RPC buscar_conhecimento_robotx só concede EXECUTE para service_role
  // (ver migration 20261008120000) — de propósito, para que só código server-side confiável
  // (esta função) consiga chamá-la.
  const supabase = createClient(supabaseUrl, serviceRoleKey);
  const openai = new OpenAI({ apiKey: openaiApiKey });

  let form: FormData;
  try {
    form = await req.formData();
  } catch (err) {
    console.error("robotx-voice: payload não é multipart/form-data válido", err);
    return jsonError(400, "corpo da requisição não é multipart/form-data válido");
  }

  const audio = form.get("audio");
  const salaId = form.get("sala_id");
  const jogoIdDoCliente = form.get("jogo_id");

  if (!(audio instanceof File) || audio.size === 0) {
    return jsonError(400, "arquivo de áudio (\"audio\") ausente, vazio ou corrompido");
  }
  if (!isUuid(salaId)) {
    return jsonError(400, "\"sala_id\" ausente ou não é um UUID válido");
  }

  // ADR 003: o jogo_id da requisição NUNCA é confiável, mesmo que o cliente o envie (o campo
  // existe no payload só porque foi pedido assim). O valor que importa é resolvido aqui, a
  // partir da sala, no servidor — é isso que torna o isolamento do RAG à prova de cliente
  // malicioso ou com bug, e não apenas "bem comportado".
  const { data: sala, error: salaError } = await supabase
    .from("salas")
    .select("jogo_id")
    .eq("id", salaId)
    .maybeSingle();

  if (salaError) {
    console.error("robotx-voice: falha ao resolver sala_id -> jogo_id", salaError);
    return jsonError(500, "falha ao resolver a sala");
  }
  if (!sala) {
    return jsonError(404, "sala não encontrada");
  }

  const jogoId = sala.jogo_id as string;
  if (isUuid(jogoIdDoCliente) && jogoIdDoCliente !== jogoId) {
    console.warn(
      `robotx-voice: jogo_id enviado pelo cliente (${jogoIdDoCliente}) difere do jogo_id ` +
        `real da sala (${jogoId}); o valor do cliente foi ignorado`,
    );
  }

  // 1. Transcrição (Whisper)
  let transcript: string;
  try {
    const transcription = await openai.audio.transcriptions.create({
      file: audio,
      model: "whisper-1",
      language: "pt",
    });
    transcript = transcription.text?.trim() ?? "";
  } catch (err) {
    console.error("robotx-voice: falha na transcrição (Whisper)", err);
    return await respondWithFallback(openai, FALLBACK_TECHNICAL_ERROR, 502);
  }

  if (!transcript) {
    return await respondWithFallback(openai, FALLBACK_EMPTY_TRANSCRIPT, 200);
  }

  // 2. Embedding da pergunta transcrita
  let embedding: number[];
  try {
    const embeddingResponse = await openai.embeddings.create({
      model: "text-embedding-3-small",
      input: transcript,
    });
    embedding = embeddingResponse.data[0].embedding;
  } catch (err) {
    console.error("robotx-voice: falha ao gerar embedding da pergunta", err);
    return await respondWithFallback(openai, FALLBACK_TECHNICAL_ERROR, 502);
  }

  // 3. Busca vetorial isolada por jogo (RPC SECURITY DEFINER; filtro jogo_id é imposto dentro
  // da própria função SQL, não só aqui — defesa em profundidade, ver migration 20261008120000).
  const { data: contexto, error: rpcError } = await supabase.rpc(
    "buscar_conhecimento_robotx",
    {
      param_jogo_id: jogoId,
      param_embedding: embedding,
      param_limite: 5,
    },
  );

  if (rpcError) {
    console.error("robotx-voice: falha na busca vetorial (RPC)", rpcError);
    return await respondWithFallback(openai, FALLBACK_TECHNICAL_ERROR, 502);
  }

  const trechos = (contexto ?? []) as { id: string; conteudo: string }[];
  if (trechos.length === 0) {
    return await respondWithFallback(openai, FALLBACK_NO_CONTEXT, 200);
  }

  const contextoTexto = trechos
    .map((trecho, index) => `[${index + 1}] ${trecho.conteudo}`)
    .join("\n\n");

  // 4. Orquestração do prompt: system prompt fixo + contexto recuperado + pergunta transcrita.
  let resposta: string;
  try {
    const completion = await openai.chat.completions.create({
      model: "gpt-4o-mini",
      temperature: 0.4,
      max_tokens: 200,
      messages: [
        { role: "system", content: SYSTEM_PROMPT },
        {
          role: "user",
          content:
            `Contexto recuperado sobre o jogo desta sala:\n\n${contextoTexto}\n\n` +
            `Pergunta do jogador: "${transcript}"`,
        },
      ],
    });
    resposta = completion.choices[0]?.message?.content?.trim() ?? "";
  } catch (err) {
    console.error("robotx-voice: falha na chat completion", err);
    return await respondWithFallback(openai, FALLBACK_TECHNICAL_ERROR, 502);
  }

  if (!resposta) {
    return await respondWithFallback(openai, FALLBACK_TECHNICAL_ERROR, 502);
  }

  // 5. Text-to-speech da resposta final.
  try {
    const speech = await openai.audio.speech.create({
      model: "tts-1",
      voice: "alloy",
      input: resposta,
    });
    const audioBuffer = await speech.arrayBuffer();

    return new Response(audioBuffer, {
      status: 200,
      headers: {
        ...CORS_HEADERS,
        "Content-Type": "audio/mpeg",
        "X-Robotx-Transcript": encodeURIComponent(transcript),
        "X-Robotx-Answer": encodeURIComponent(resposta),
      },
    });
  } catch (err) {
    console.error("robotx-voice: falha no TTS da resposta final", err);
    // O texto da resposta já existe (a LLM respondeu); só o áudio falhou. Devolve o texto em
    // vez de inventar um áudio quebrado ou silencioso.
    return jsonError(502, "falha ao gerar áudio de resposta", { answer_text: resposta });
  }
});
