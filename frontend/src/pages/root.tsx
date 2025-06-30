import { CSSProperties } from "react";
import { Routes, Route } from "react-router-dom";

import { SidebarProvider } from "@/components/ui/sidebar";

import { AppSidebar } from "@/sidebar";

import { JournalRoutes } from "@/pages/journals";
import { AdminRoutes } from "@/pages/admin";
import { SettingsRoutes } from "@/pages/settings";
import { CenterPage } from "@/components/ui/page";

export function Root() {
    return <div className="flex flex-row flex-nowrap w-full h-full">
        <SidebarProvider style={{"--sidebar-width": "350px"} as CSSProperties}>
            <AppSidebar/>
            <main className="relative flex-auto overflow-auto">
                <div className="px-2 h-full">
                    <Routes>
                        <Route index element={<NothingToSee/>}/>
                        <Route path="/journals/*" element={<JournalRoutes />}/>
                        <Route path="/settings/*" element={<SettingsRoutes />}/>
                        <Route path="/admin/*" element={<AdminRoutes />}/>
                    </Routes>
                </div>
            </main>
        </SidebarProvider>
    </div>;
}

function NothingToSee() {
    return <CenterPage className="flex items-center justify-center h-full">
        <div className="w-1/2 flex flex-col flex-nowrap items-center">
            <h2 className="text-2xl">Nothing to see here</h2>
            <p className="text-center">There might be something here in the future but for now it is only the VOID!</p>
        </div>
    </CenterPage>;
}
