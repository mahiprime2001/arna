import { createApp } from "vue";
import ConsentApp from "./ConsentApp.vue";
import ChatApp from "./ChatApp.vue";
import PairApp from "./PairApp.vue";
import "./style.css";

// One frontend, three windows: the consent popup (default), the chat window
// (?view=chat), and the first-run pairing window (?view=pair).
const view = new URLSearchParams(location.search).get("view");
const App = view === "chat" ? ChatApp : view === "pair" ? PairApp : ConsentApp;
createApp(App).mount("#app");
