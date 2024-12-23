import { ReactElement } from "react";
import { Link, Routes, Route, useNavigate, useLocation } from "react-router-dom";
import {
    BadgeCheck,
    Bell,
    ChevronsUpDown,
    LogOut,
    Notebook,
    Settings,
    Shield,
    EarthLock,
    Users,
} from "lucide-react";

import {
    Avatar,
    AvatarFallback,
    AvatarImage,
} from "@/components/ui/avatar";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
import { JournalSidebar } from "@/journals/sidebar";
import { AdminSidebar } from "@/admin";

async function send_logout() {
    let res = await fetch("/logout", {
        method: "POST"
    });

    if (res.status != 200) {
        throw new Error("failed to logout user");
    }
}

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

function JournalGroup() {
    return <SidebarGroup>
        <SidebarGroupLabel>Journals</SidebarGroupLabel>
        <SidebarGroupContent>
            <SidebarMenu>
                <MenuOption title="All Journals" path="/journals"/>
            </SidebarMenu>
        </SidebarGroupContent>
    </SidebarGroup>
}

function AdministrativeGroup() {
    return <SidebarGroup>
        <SidebarGroupLabel>Administrative</SidebarGroupLabel>
        <SidebarGroupContent>
            <SidebarMenu>
                <MenuOption title="Users" path="/users"/>
                <MenuOption title="Groups" path="/groups"/>
                <MenuOption title="Roles" path="/roles"/>
            </SidebarMenu>
        </SidebarGroupContent>
    </SidebarGroup>
}

interface UserBadgeProps {
    name: string,
    email: string,
    avatar?: string,
}

function UserBadge({name, email, avatar}: UserBadgeProps) {
    return <>
        <Avatar className="h-8 w-8 rounded-lg">
            <AvatarImage src={avatar} alt={name} />
            <AvatarFallback className="rounded-lg">TJ2</AvatarFallback>
        </Avatar>
        <div className="grid flex-1 text-left text-sm leading-tight">
            <span className="truncate font-semibold">{name}</span>
            <span className="truncate text-xs">{email}</span>
        </div>
    </>;
}

interface UserMenuProps {
    name: string,
    email: string,
    avatar?: string,
}

function UserMenu({name, email, avatar}: UserMenuProps) {
    const { isMobile } = useSidebar();
    const navigate = useNavigate();

    return <SidebarMenu>
        <SidebarMenuItem>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <SidebarMenuButton
                        size="lg"
                        className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground md:h-8 md:p-0"
                    >
                        <UserBadge name={name} email={email} avatar={avatar}/>
                        <ChevronsUpDown className="ml-auto size-4" />
                    </SidebarMenuButton>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                    className="w-[--radix-dropdown-menu-trigger-width] min-w-56 rounded-lg"
                    side={isMobile ? "bottom" : "right"}
                    align="end"
                    sideOffset={4}
                >
                    <DropdownMenuLabel className="p-0 font-normal">
                        <div className="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
                            <UserBadge name={name} email={email} avatar={avatar}/>
                        </div>
                    </DropdownMenuLabel>
                    <DropdownMenuItem onSelect={(ev) => {
                        send_logout().then(() => {
                            navigate("/login");
                        }).catch(err => {
                            console.error("failed to logout:", err);
                        });
                    }}>
                        <LogOut />
                        Log out
                    </DropdownMenuItem>
                    <DropdownMenuSeparator/>
                    <Link to="/settings">
                        <DropdownMenuItem>
                            Settings
                        </DropdownMenuItem>
                    </Link>
                </DropdownMenuContent>
            </DropdownMenu>
        </SidebarMenuItem>
    </SidebarMenu>;
}

interface NavSidebarProps {
    name: string,
    email: string,
    avatar?: string,
}

function NavSidebar({name, email, avatar}: NavSidebarProps) {
    const location = useLocation();

    return <Sidebar
        collapsible="none"
        className="!w-[calc(var(--sidebar-width-icon)_+_1px)] border-r"
    >
        <SidebarHeader>
            <UserMenu name={name} email={email} avatar={avatar}/>
        </SidebarHeader>
        <SidebarContent>
            <SidebarGroup>
                <SidebarGroupContent className="px-1.5 md:px-0">
                    <SidebarMenu>
                        <MenuOption
                            title="Journals"
                            path="/journals"
                            active={location.pathname.startsWith("/journals")}
                            icon={<Notebook />}
                        />
                        <MenuOption
                            title="Administrative"
                            path="/admin"
                            active={location.pathname.startsWith("/admin")}
                            icon={<EarthLock />}
                        />
                        <MenuOption
                            title="Settings"
                            path="/settings"
                            active={location.pathname.startsWith("/settings")}
                            icon={<Settings />}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
    </Sidebar>
}

function AppSidebar() {
    return <Sidebar
        collapsible="icon"
        className="overflow-hidden [&>[data-sidebar=sidebar]]:flex-row"
    >
        <NavSidebar name="The Dude" email="the_dude@laboski.drink"/>
        <Sidebar collapsible="none" className="hidden flex-1 md:flex">
            <Routes>
                <Route path="/journals/*" element={<JournalSidebar />}/>
                <Route path="/admin/*" element={<AdminSidebar />}/>
            </Routes>
        </Sidebar>
    </Sidebar>
}

export { AppSidebar };
