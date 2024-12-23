import { ReactElement } from "react";
import { Link, Routes, Route, useLocation } from "react-router-dom";
import {
    BadgeCheck,
    Bell,
    ChevronsUpDown,
    LogOut,
    Notebook,
    Settings,
    Shield,
    EarthLock,
    Users as UsersIcon,
} from "lucide-react";

import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupLabel,
    SidebarGroupContent,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuItem,
    SidebarMenuButton,
} from "@/components/ui/sidebar";

import { Groups, Group } from "@/groups";
import { Roles, Role } from "@/roles";
import { Users, User } from "@/users";

interface MenuOptionProps {
    title: string,
    path: string,
    active?: boolean,
    icon?: ReactElement,
}

function MenuOption({title, path, icon, active = false}: MenuOptionProps) {
    return <SidebarMenuItem>
        <SidebarMenuButton
            asChild
            tooltip={{children: title, hidden: false}}
            isActive={active}
            className="px-2.5 md:px-2"
        >
            <Link to={path}>
                {icon}
                <span>{title}</span>
            </Link>
        </SidebarMenuButton>
    </SidebarMenuItem>
}

function AdminSidebar() {
    const location = useLocation();

    return <>
        <SidebarContent>
            <SidebarGroup>
                <SidebarGroupLabel>User Management</SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <MenuOption
                            title="Users"
                            path="/admin/users"
                            active={location.pathname.startsWith("/admin/users")}
                            icon={<UsersIcon />}
                        />
                        <MenuOption
                            title="Groups"
                            path="/admin/groups"
                            active={location.pathname.startsWith("/admin/groups")}
                        />
                        <MenuOption
                            title="Roles"
                            path="/admin/roles"
                            active={location.pathname.startsWith("/admin/roles")}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
    </>;
}

function AdminRoutes() {
    return <Routes>
        <Route index element={<span>Admin Index</span>}/>
        <Route path="/users" element={<Users />}/>
        <Route path="/users/:users_id" element={<User />}/>
        <Route path="/groups" element={<Groups />}/>
        <Route path="/groups/:groups_id" element={<Group />}/>
        <Route path="/roles" element={<Roles />}/>
        <Route path="/roles/:role_id" element={<Role />}/>
    </Routes>;
}

export { AdminRoutes, AdminSidebar };
