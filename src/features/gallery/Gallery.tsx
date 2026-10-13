import { Chip, Empty } from "../../components/Bits";

export function Gallery() {
  return (
    <div className="content">
      <h1>Component gallery</h1>
      <p className="tiny">Development reference for the tokens in docs/DESIGN_SYSTEM.md. These controls are the same ones used in the app.</p>
      <div className="row">
        <button className="btn">Primary</button>
        <button className="btn-ghost">Secondary</button>
        <button className="btn-danger">Danger</button>
        <Chip kind="ok">Ready</Chip>
        <Chip kind="warn">Needs setup</Chip>
        <Chip kind="bad">Missing</Chip>
        <span className="kbd">Alt</span>
        <span className="kbd">Space</span>
      </div>
      <Empty title="No preview available" body="This is the honest empty state used when an app has no user-supplied screenshot." />
    </div>
  );
}
