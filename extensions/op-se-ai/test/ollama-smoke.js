const { completionUrl, streamCompletion } = require('../provider');

async function main() {
  const model = process.env.OP_SE_TEST_MODEL;
  if (!model) throw new Error('Set OP_SE_TEST_MODEL to an installed local Ollama model');
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 120000);
  let chunks = 0;
  let answer = '';
  try {
    await streamCompletion({
      url: completionUrl('http://127.0.0.1:11434/v1'),
      model,
      messages: [{ role: 'user', content: 'Reply with only the word OK.' }],
      maxTokens: 128,
      signal: controller.signal,
      onDelta(chunk) { chunks++; answer += chunk; }
    });
  } finally {
    clearTimeout(timeout);
  }
  if (!chunks || !answer.trim()) throw new Error('The local model returned no streamed text');
  console.log(`Ollama streamed ${chunks} text chunks: ${answer.trim().slice(0, 80)}`);
}

main().catch(error => { console.error(error.message); process.exitCode = 1; });
