import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";

import Login from "./Login";
import App from "./App";

document.addEventListener("DOMContentLoaded", () => {
    const root = document.getElementById("root");
    const renderer = createRoot(root);

    renderer.render(
        <BrowserRouter basename="/">
            <Routes>
                <Route path="/login" element={<Login/>}/>
                <Route path="*" element={<App/>}/>
            </Routes>
        </BrowserRouter>
    );
});
