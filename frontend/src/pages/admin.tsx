import { Routes, Route } from "react-router-dom";

import { Groups } from "@/pages/admin/groups";
import { Group } from "@/pages/admin/groups/groups_id";
import { Roles } from "@/pages/admin/roles";
import { Role } from "@/pages/admin/roles/role_id";
import { Users } from "@/pages/admin/users";
import { User } from "@/pages/admin/users/users_id";
import { InviteTable } from "@/pages/admin/invites";
import { Invite } from "@/pages/admin/invites/invite_token";

export function AdminRoutes() {
    return <Routes>
        <Route index element={<span>Admin Index</span>}/>
        <Route path="/users" element={<Users />}/>
        <Route path="/users/:users_id" element={<User />}/>
        <Route path="/groups" element={<Groups />}/>
        <Route path="/groups/:groups_id" element={<Group />}/>
        <Route path="/roles" element={<Roles />}/>
        <Route path="/roles/:role_id" element={<Role />}/>
        <Route path="/invites" element={<InviteTable />}/>
        <Route path="/invites/:token" element={<Invite />}/>
    </Routes>;
}
