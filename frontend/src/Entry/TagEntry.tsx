import { useFieldArray, useFormContext  } from "react-hook-form";
import { Trash, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
} from "@/components/ui/form";
import { EntryForm } from "@/journal";

export default function TagEntry() {
    const form = useFormContext<EntryForm>();
    const tags = useFieldArray<EntryForm, "tags">({
        control: form.control,
        name: "tags"
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Tags
            <Button type="button" variant="secondary" onClick={() => {
                tags.append({key: "", value: ""});
            }}>Add Tag<Plus/></Button>
        </div>
        {tags.fields.map((field, index) => {
            return <div key={field.id} className="flex flex-row flex-nowrap gap-x-4">
                <FormField control={form.control} name={`tags.${index}.key`} render={({field: tag_field}) => {
                    return <FormItem className="w-1/4">
                        <FormControl>
                            <Input type="text" {...tag_field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <FormField control={form.control} name={`tags.${index}.value`} render={({field: tag_field}) => {
                    return <FormItem className="w-2/4">
                        <FormControl>
                            <Input type="text" {...tag_field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <Button type="button" variant="destructive" size="icon" onClick={() => {
                    tags.remove(index);
                }}><Trash/></Button>
            </div>
        })}
    </div>
}
