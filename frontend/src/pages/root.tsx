import { BrowserRouter, Routes, Route } from "react-router-dom";

import {
    SidebarProvider,
    SidebarTrigger,
} from "@/components/ui/sidebar";

import { AppSidebar } from "@/sidebar";

import { JournalRoutes } from "@/journals";
import { AdminRoutes } from "@/admin";
import { SettingsRoutes } from "@/pages/settings";

export function Root() {
    return <div className="flex flex-row flex-nowrap w-full h-full">
        <SidebarProvider style={{"--sidebar-width": "350px"}}>
            <AppSidebar/>
            <main className="relative flex-auto overflow-auto">
                <div className="px-2 px-2 h-full">
                    <Routes>
                        <Route index element={<div>root page</div>}/>
                        <Route path="/journals/*" element={<JournalRoutes />}/>
                        <Route path="/settings/*" element={<SettingsRoutes />}/>
                        <Route path="/admin/*" element={<AdminRoutes />}/>
                    </Routes>
                </div>
            </main>
        </SidebarProvider>
    </div>;
}
