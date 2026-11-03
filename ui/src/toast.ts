import { mount } from "svelte";
import "./app.css";
import Nag from "./Nag.svelte";

export default mount(Nag, { target: document.getElementById("app")!, props: { mode: "toast" } });
