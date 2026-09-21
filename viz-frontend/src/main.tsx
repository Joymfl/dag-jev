import { render } from "preact";
import "./index.css";
import { App } from "./app.tsx";
import cytoscape from "cytoscape";
import elk from "cytoscape-elk";
cytoscape.use(elk);

render(<App />, document.getElementById("app")!);
