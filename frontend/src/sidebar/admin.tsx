import { ReactElement } from "react";
import { Link, Routes, Route, useLocation } from "react-router-dom";
import { Users as UsersIcon } from "lucide-react";

import {
    SidebarContent,
    SidebarGroup,
    SidebarGroupLabel,
    SidebarGroupContent,
    SidebarMenu,
    SidebarMenuLink,
} from "@/components/ui/sidebar";

export function AdminSidebar() {
    const location = useLocation();

    return <>
        <SidebarContent>
            <SidebarGroup>
                <SidebarGroupLabel>User Management</SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Users"
                            path="/admin/users"
                            active={location.pathname.startsWith("/admin/users")}
                            icon={<UsersIcon />}
                        />
                        <SidebarMenuLink
                            title="Groups"
                            path="/admin/groups"
                            active={location.pathname.startsWith("/admin/groups")}
                        />
                        <SidebarMenuLink
                            title="Roles"
                            path="/admin/roles"
                            active={location.pathname.startsWith("/admin/roles")}
                        />
                        <SidebarMenuLink
                            title="Invites"
                            path="/admin/invites"
                            active={location.pathname.startsWith("/admin/invites")}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
    </>;
}
