import { BrowserRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { Login } from "@/pages/login";
import { Register } from "@/pages/register";
import { Verify } from "@/pages/verify";
import { Root } from "@/pages/root";
import { Toaster } from "./components/ui/toaster";

const query_client = new QueryClient();

export function MainRouter() {
    return <QueryClientProvider client={query_client}>
        <BrowserRouter basename="/">
            <Routes>
                <Route path="/login" element={<Login/>}/>
                <Route path="/verify" element={<Verify />}/>
                <Route path="/register" element={<Register />}/>
                <Route path="*" element={<Root/>}/>
            </Routes>
        </BrowserRouter>
        <Toaster/>
    </QueryClientProvider>;
}
