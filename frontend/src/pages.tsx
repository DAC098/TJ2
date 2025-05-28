import { BrowserRouter, Routes, Route } from "react-router-dom";

import {
    SidebarProvider,
    SidebarTrigger,
} from "@/components/ui/sidebar";

import { Login } from "@/pages/login";
import { Register } from "@/pages/register";
import { Verify } from "@/pages/verify";
import { Root } from "@/pages/root";

export function MainRouter() {
    return <BrowserRouter basename="/">
        <Routes>
            <Route path="/login" element={<Login/>}/>
            <Route path="/verify" element={<Verify />}/>
            <Route path="/register" element={<Register />}/>
            <Route path="*" element={<Root/>}/>
        </Routes>
    </BrowserRouter>;
}
