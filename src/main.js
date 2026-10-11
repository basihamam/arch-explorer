import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

const app = mount(App, { target: document.getElementById("app") });
// App.tsx / main.js
window.addEventListener('DOMContentLoaded', () => {
  document.body.style.cursor = 'default';
});
export default app;
