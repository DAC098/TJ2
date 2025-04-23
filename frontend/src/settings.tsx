import { useState, useEffect, Fragment } from "react";
import { Link, useLocation, Routes, Route } from "react-router-dom";
import { Shield } from "lucide-react";

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
    useSidebar,
} from "@/components/ui/sidebar";
import { cn } from "@/utils";

import { Auth } from "@/settings/auth";

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

export function SettingsSidebar() {
    const location = useLocation();

    return <SidebarContent>
        <SidebarGroup>
            <SidebarGroupLabel>Account Management</SidebarGroupLabel>
            <SidebarGroupContent>
                <SidebarMenu>
                    <MenuOption
                        title="Authentication"
                        path="/settings/auth"
                        active={location.pathname === "/settings/auth"}
                        icon={<Shield />}
                    />
                </SidebarMenu>
            </SidebarGroupContent>
        </SidebarGroup>
    </SidebarContent>
}

export function SettingsRoutes() {
    return <Routes>
        <Route index element={<span>Settings Index</span>}/>
        <Route path="/auth" element={<Auth />}/>
    </Routes>
}
