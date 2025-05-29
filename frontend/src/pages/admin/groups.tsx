import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler } from "react-hook-form";
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
import { UserList, AttachedUser } from "@/components/users";
import { RoleList, AttachedRole } from "@/roles";

export interface GroupPartial {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null
}

async function get_groups() {
    let res = await fetch("/admin/groups");

    if (res.status !== 200) {
        console.log("non 200 response status:", res);

        return null;
    }

    return await res.json() as GroupPartial[];
}

export function Groups() {
    let [loading, set_loading] = useState(false);
    let [groups, set_groups] = useState<GroupPartial[]>([]);

    const columns: ColumnDef<GroupPartial>[] = [
        {
            header: "Name",
            cell: ({ row }) => {
                return <Link to={`/admin/groups/${row.original.id}`}>{row.original.name}</Link>;
            }
        },
        {
            header: "Mod",
            cell: ({ row }) => {
                return row.original.updated != null ? row.original.updated : row.original.created;
            }
        }
    ];

    useEffect(() => {
        set_loading(true);

        get_groups().then(list => {
            if (list == null) {
                return;
            }

            set_groups(list);
        }).catch(err => {
            console.error("failed to load group list", err);
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to="/admin/groups/new">
                <Button type="button">New Group<Plus /></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={groups}/>
    </div>;
}
