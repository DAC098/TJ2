import { BrowserRouter, Routes, Route } from "react-router-dom";

import {
    SidebarProvider,
    SidebarTrigger,
} from "@/components/ui/sidebar";

import { AppSidebar } from "@/sidebar";
import { JournalRoutes } from "@/journals";
import { AdminRoutes } from "@/admin";

function App() {
    return <div className="flex flex-row flex-nowrap w-full h-full">
        <SidebarProvider style={{"--sidebar-width": "350px"}}>
            <AppSidebar/>
            <main className="relative flex-auto overflow-auto">
                <div className="pt-2 pr-2 pl-2 h-full">
                    <Routes>
                        <Route index element={<div>root page</div>}/>
                        <Route path="/journals/*" element={<JournalRoutes />}/>
                        <Route path="/admin/*" element={<AdminRoutes />}/>
                    </Routes>
                </div>
            </main>
        </SidebarProvider>
    </div>;
}

export default App;
