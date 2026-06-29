import { createApp } from "vue";
import "./style.css";
import App from "./App.vue";
import ConsentApp from "./ConsentApp.vue";
import ChatApp from "./ChatApp.vue";
import PairApp from "./PairApp.vue";

// One frontend, several windows. The main window is the console (control others);
// the Rust side opens secondary windows for the agent side of things with a
// `?view=` query: the consent popup, the in-session chat window, and the pairing
// form that makes this PC reachable.
const view = new URLSearchParams(location.search).get("view");
const Root =
  view === "consent" ? ConsentApp : view === "chat" ? ChatApp : view === "pair" ? PairApp : App;

createApp(Root).mount("#app");
