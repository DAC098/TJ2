import { useFieldArray, useFormContext  } from "react-hook-form";

import { EntryForm } from "../journal";

export default function TagEntry() {
    const form = useFormContext<EntryForm>();
    const tags = useFieldArray<EntryForm, "tags">({
        control: form.control,
        name: "tags"
    });

    return <>
        <button type="button" onClick={() => {
            tags.append({key: "", value: ""});
        }}>Add</button>
        {tags.fields.map((field, index) => {
            return <div key={field.id}>
                <button type="button" onClick={() => {
                    tags.remove(index);
                }}>Drop</button>
                <input type="text" {...form.register(`tags.${index}.key`)}/>
                <input type="text" {...form.register(`tags.${index}.value`)}/>
            </div>
        })}
    </>
}
