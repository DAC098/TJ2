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
    SidebarMenuLink,
    useSidebar,
} from "@/components/ui/sidebar";
import { cn } from "@/utils";

import { Auth } from "@/pages/settings/auth";
import { PeerClient } from "@/pages/settings/peer_client";

export function SettingsSidebar() {
    const location = useLocation();

    return <SidebarContent>
        <SidebarGroup>
            <SidebarGroupLabel>Account Management</SidebarGroupLabel>
            <SidebarGroupContent>
                <SidebarMenu>
                    <SidebarMenuLink
                        title="Authentication"
                        path="/settings/auth"
                        active={location.pathname === "/settings/auth"}
                        icon={<Shield />}
                    />
                    <SidebarMenuLink
                        title="Peers / Clients"
                        path="/settings/peer_client"
                        active={location.pathname === "/settings/peer_client"}
                    />
                </SidebarMenu>
            </SidebarGroupContent>
        </SidebarGroup>
    </SidebarContent>
}
