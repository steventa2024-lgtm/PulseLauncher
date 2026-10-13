import { Banner, Button, EmptyState, Kbd, StatusBadge } from "../components/ui";
import { Highlight } from "../components/ui";

export function GalleryView() {
  return (
    <section className="space-y-4">
      <header>
        <h1 className="text-2xl font-semibold">Design gallery</h1>
        <p className="text-sm text-muted">Component states for the dark-blue studio. The sample card is not an installed application.</p>
      </header>
      <div className="flex flex-wrap gap-2">
        <Button variant="primary">Primary</Button>
        <Button>Secondary</Button>
        <Button variant="danger">Danger</Button>
        <Button disabled>Disabled</Button>
      </div>
      <div className="flex flex-wrap gap-2">
        <StatusBadge status="ready" />
        <StatusBadge status="needsSetup" />
        <StatusBadge status="unavailable" />
        <StatusBadge status="missing" />
      </div>
      <p className="text-sm">
        Keyboard <Kbd>Alt</Kbd> + <Kbd>Space</Kbd> · highlight <Highlight text="PulseLauncher" indices={[0, 1, 2, 3, 4]} />
      </p>
      <Banner>Informational banner</Banner>
      <Banner tone="warn">Warning banner</Banner>
      <Banner tone="danger">Error banner</Banner>
      <EmptyState title="Empty state" body="Use this when a scan or filter has nothing to show. Do not fill it with fictional apps." />
      <article className="max-w-sm rounded-card border border-line bg-panel p-3">
        <p className="text-xs uppercase tracking-wide text-bright">Sample only</p>
        <h2 className="font-semibold">Sample card</h2>
        <p className="text-xs text-muted">This card is a design specimen. It does not launch software.</p>
      </article>
    </section>
  );
}
