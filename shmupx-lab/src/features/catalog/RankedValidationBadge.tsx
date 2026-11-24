import Badge from "../../components/ui/Badge";
import type { RankedValidation } from "../../lib/rankedValidation";

/** Como o Client valida o Ranked do jogo: pela memória, pelo script de vídeo, ou ainda não valida. */
export default function RankedValidationBadge({ validation }: { validation: RankedValidation | undefined }) {
  const roms = validation?.roms ?? [];
  const script = validation?.script ?? null;
  if (roms.length === 0 && !script) {
    return <span title="O Client ainda não valida as runs: Coming soon, só Training Mode."><Badge>Coming soon</Badge></span>;
  }
  return (
    <span className="inline-flex gap-1">
      {roms.length > 0 && (
        <span title={`O ShmupX Emulator lê o placar da memória do jogo (${roms.join(", ")}): não precisa de script de vídeo.`}>
          <Badge tone="info">Memória</Badge>
        </span>
      )}
      {script && (
        <span title={`Script de vídeo ${script} aprovado e aceito pelo Client.`}>
          <Badge tone="success">Script {script}</Badge>
        </span>
      )}
    </span>
  );
}
