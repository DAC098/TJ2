import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";

import { App } from "@/App";
import { Login } from "@/Login";
import { Register } from "@/register";
import { Verify } from "@/verify";

import "@/media";

function get_window_theme() {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function get_theme_preference() {
    if (typeof localStorage != null) {
        let theme = localStorage.getItem("theme");

        return theme == null ? get_window_theme() : theme;
    } else {
        return get_window_theme();
    }
}

if (get_theme_preference() === "dark") {
    document.documentElement.classList.add("dark");
}

document.addEventListener("DOMContentLoaded", () => {
    const root = document.getElementById("root");
    const renderer = createRoot(root);

    renderer.render(
        <BrowserRouter basename="/">
            <Routes>
                <Route path="/login" element={<Login/>}/>
                <Route path="/verify" element={<Verify />}/>
                <Route path="/register" element={<Register />}/>
                <Route path="*" element={<App/>}/>
            </Routes>
        </BrowserRouter>
    );
});
