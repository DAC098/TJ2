import { BrowserRouter, Routes, Route } from "react-router-dom";

import {
    SidebarProvider,
    SidebarTrigger,
} from "@/components/ui/sidebar";

import Entries from "@/Entries";
import Entry from "@/Entry";
import { Users } from "@/users";
import { AppSidebar } from "@/sidebar";

const App = () => {
    return <div className="flex flex-row flex-nowrap w-full h-full">
        <SidebarProvider>
            <AppSidebar/>
            <main className="relative flex-auto overflow-auto">
                <Routes>
                    <Route path="/" element={<div>root page</div>}/>
                    <Route path="/entries" element={<Entries />}/>
                    <Route path="/entries/:entry_date" element={<Entry />}/>
                    <Route path="/users" element={<Users />}/>
                    <Route path="/users/:user_id" element={<>specific user</>}/>
                </Routes>
            </main>
        </SidebarProvider>
    </div>
};

export default App;
