import { BrowserRouter, Routes, Route } from "react-router-dom";

import {
    SidebarProvider,
    SidebarTrigger,
} from "@/components/ui/sidebar";

import { Groups, Group } from "@/groups";
import { Roles, Role } from "@/roles";
import { AppSidebar } from "@/sidebar";
import { Users, User } from "@/users";
import { JournalRoutes } from "@/journals";

function App() {
    return <div className="flex flex-row flex-nowrap w-full h-full">
        <SidebarProvider>
            <AppSidebar/>
            <main className="relative flex-auto overflow-auto">
                <div className="pt-2 pr-2 pl-2">
                    <Routes>
                        <Route path="/" element={<div>root page</div>}/>
                        <Route path="/journals/*" element={<JournalRoutes />}/>
                        <Route path="/users" element={<Users />}/>
                        <Route path="/users/:users_id" element={<User />}/>
                        <Route path="/groups" element={<Groups />}/>
                        <Route path="/groups/:groups_id" element={<Group />}/>
                        <Route path="/roles" element={<Roles />}/>
                        <Route path="/roles/:role_id" element={<Role />}/>
                    </Routes>
                </div>
            </main>
        </SidebarProvider>
    </div>
}

export default App;
