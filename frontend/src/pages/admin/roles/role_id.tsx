import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Separator } from "@/components/ui/separator";
import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetHeader,
    SheetTitle,
    SheetTrigger,
} from "@/components/ui/sheet";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { GroupList, AttachedGroup } from "@/components/groups";
import { UserList, AttachedUser } from "@/components/users";

interface RoleFull {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null,
    permissions: AttachedPermission[],
    users: AttachedUser[],
    groups: AttachedGroup[],
}

interface AttachedPermission {
    scope: string,
    ability: string,
    added: string,
}

interface RoleForm {
    name: string,
    permissions: RolePermissions,
    users: AttachedUser[],
    groups: AttachedGroup[],
}

interface RolePermissions {
    journals: Abilities,
    entries: Abilities,
    users: Abilities,
    groups: Abilities,
    roles: Abilities,
}

interface Abilities {
    create: boolean,
    read: boolean,
    update: boolean,
    delete: boolean,
}

function role_to_form(role: RoleFull): RoleForm {
    let rtn = {
        name: role.name,
        permissions: {
            journals: abilities_object(),
            entries: abilities_object(),
            users: abilities_object(),
            groups: abilities_object(),
            roles: abilities_object(),
        },
        users: role.users,
        groups: role.groups,
    };

    for (let perm of role.permissions) {
        if (perm.scope in rtn.permissions && perm.ability in rtn.permissions[perm.scope]) {
            rtn.permissions[perm.scope][perm.ability] = true;
        } else {
            console.log("permission not in permissions object");
        }
    }

    return rtn;
}

function blank_form(): RoleForm {
    return {
        name: "",
        permissions: {
            journals: abilities_object(),
            entries: abilities_object(),
            users: abilities_object(),
            groups: abilities_object(),
            roles: abilities_object(),
        },
        users: [],
        groups: [],
    };
}

function abilities_object(): Abilities {
    return {
        create: false,
        read: false,
        update: false,
        delete: false,
    };
}

async function get_role(role_id: string) {
    let res = await fetch(`/admin/roles/${role_id}`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as RoleFull;
}

interface RoleHeaderProps {
    role_id: string,
    on_delete: () => void,
}

function RoleHeader({role_id, on_delete}: RoleHeaderProps) {
    const form = useFormContext<RoleForm>();

    return <div className="flex flex-row flex-nowrap gap-x-4 items-center">
        <FormField control={form.control} name="name" render={({field}) => {
            return <FormItem className="w-1/2">
                <FormControl>
                    <Input type="text" placeholder="Name" {...field}/>
                </FormControl>
            </FormItem>
        }}/>
        <Button type="submit">Save<Save/></Button>
        {role_id !== "new" ?
            <Button
                type="button"
                variant="destructive"
                onClick={() => {
                    on_delete();
                }}
            >
                Delete
                <Trash/>
            </Button>
            :
            null
        }
    </div>;
}

interface RolePermissionJson {
    scope: string,
    abilities: string[],
}

interface RoleJson {
    name: string,
    permissions: RolePermissionJson[],
    users: string | number[],
    groups: string | number[],
}

function form_to_body(form: RoleForm) {
    let rtn: RoleJson = {
        name: form.name,
        permissions: [],
        users: form.users.map(attached => {
            return attached.users_id;
        }),
        groups: form.groups.map(attached => {
            return attached.groups_id
        }),
    };

    for (let key in form.permissions) {
        let abilities = [];

        for (let ability in form.permissions[key]) {
            if (form.permissions[key][ability]) {
                abilities.push(ability);
            }
        }

        rtn.permissions.push({
            scope: key,
            abilities,
        });
    }

    return rtn;
}

export function Role() {
    const { role_id } = useParams();
    const navigate = useNavigate();

    if (role_id == null) {
        throw new Error("role_id is null");
    }

    const form = useForm<RoleForm>({
        defaultValues: async () => {
            if (role_id === "new") {
                return blank_form();
            }

            try {
                let result = await get_role(role_id);

                if (result != null) {
                    return role_to_form(result);
                }
            } catch (err) {
                console.error("failed to retrieve role", err);
            }

            return blank_form();
        }
    });

    const create_role = async (data: RoleForm) => {
        let body = JSON.stringify(form_to_body(data));

        let res = await fetch("/admin/roles", {
            method: "POST",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return await res.json();
        case 400:
            let json = await res.json();

            console.error("failed to create role", json);
            break;
        case 403:
            console.error("you do not have permission to create roles");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return null;
    };

    const update_role = async (role_id: string, data: RoleForm) => {
        let body = JSON.stringify(form_to_body(data));

        let res = await fetch(`/admin/roles/${role_id}`, {
            method: "PATCH",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return true;
        case 400:
            let json = await res.json();

            console.error("failed to update role", json);
            break;
        case 403:
            console.error("you do not have permission to update roles");
            break;
        case 404:
            console.error("role not found");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return false;
    };

    const on_submit: SubmitHandler<RoleForm> = async (data, event) => {
        if (role_id === "new") {
            try {
                let created = await create_role(data);

                if (created == null) {
                    return;
                }

                form.reset(role_to_form(created));

                navigate(`/admin/roles/${created.id}`);
            } catch (err) {
                console.error("error when creating new role", err);
            }
        } else {
            try {
                if (await update_role(role_id, data)) {
                    form.reset(data);
                }
            } catch (err) {
                console.error("error when updating role", err);
            }
        }
    };

    const delete_role = async () => {
        if (role_id === "new") {
            return;
        }

        try {
            let res = await fetch(`/admin/roles/${role_id}`, {
                method: "DELETE",
            });

            switch (res.status) {
            case 200:
                navigate("/admin/roles");
                break;
            case 403:
                console.error("you do not have permission to delete roles");
                break;
            case 404:
                console.error("role not found");
                break;
            default:
                console.warn("unhandled response status code");
                break;
            }
        } catch (err) {
            console.error("error when deleting role", err);
        }
    };

    if (form.formState.isLoading) {
        return <div className="max-w-3xl mx-auto my-auto">
            Loading
        </div>;
    }

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<RoleForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <RoleHeader role_id={role_id} on_delete={delete_role}/>
                <Separator/>
                Permissions
                <PermissionGroup id="journals" title="Journals"/>
                <PermissionGroup id="entries" title="Entries"/>
                <PermissionGroup id="users" title="Users"/>
                <PermissionGroup id="groups" title="Groups"/>
                <PermissionGroup id="roles" title="Roles"/>
                <Separator/>
                <div className="flex flex-row gap-x-4">
                    <UserList />
                    <GroupList />
                </div>
            </form>
        }/>

    </div>;
}

interface PermissionGroupProps {
    id: string,
    title: string,
    description?: string
}

function PermissionGroup({id, title, description}: PermissionGroupProps) {
    let form = useFormContext();

    return <div className="space-y-4">
        <FormLabel>{title}</FormLabel>
        {description ?
            <FormDescription>{description}</FormDescription>
            :
            null
        }
        <div className="flex flex-row gap-4">
            <FormField control={form.control} name={`permissions.${id}.create`} render={({ field }) => {
                let {value, onChange, ...rest} = field;

                return <FormItem className="flex flex-row items-center space-y-0 space-x-2">
                    <FormControl>
                        <Checkbox checked={value} onCheckedChange={onChange} {...rest}/>
                    </FormControl>
                    <FormLabel className="space-y-0 space-x-2">Create</FormLabel>
                </FormItem>
            }}/>
            <FormField control={form.control} name={`permissions.${id}.read`} render={({ field }) => {
                let {value, onChange, ...rest} = field;

                return <FormItem className="flex flex-row items-center space-y-0 space-x-2">
                    <FormControl>
                        <Checkbox checked={value} onCheckedChange={onChange} {...rest}/>
                    </FormControl>
                    <FormLabel>Read</FormLabel>
                </FormItem>
            }}/>
            <FormField control={form.control} name={`permissions.${id}.update`} render={({ field }) => {
                let {value, onChange, ...rest} = field;

                return <FormItem className="flex flex-row items-center space-y-0 space-x-2">
                    <FormControl>
                        <Checkbox checked={value} onCheckedChange={onChange} {...rest}/>
                    </FormControl>
                    <FormLabel>Update</FormLabel>
                </FormItem>
            }}/>
            <FormField control={form.control} name={`permissions.${id}.delete`} render={({ field }) => {
                let {value, onChange, ...rest} = field;

                return <FormItem className="flex flex-row items-center space-y-0 space-x-2">
                    <FormControl>
                        <Checkbox checked={value} onCheckedChange={onChange} {...rest}/>
                    </FormControl>
                    <FormLabel>Delete</FormLabel>
                </FormItem>
            }}/>
        </div>
    </div>;
}
