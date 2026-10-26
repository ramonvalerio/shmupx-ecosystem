export default function JogosListSkeleton() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {Array.from({ length: 3 }).map((_, index) => (
        <div
          key={index}
          className="h-32 animate-pulse rounded-lg border border-white/10 bg-zinc-900/40"
        />
      ))}
    </div>
  );
}
