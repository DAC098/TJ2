import { useState, useEffect, JSX } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useForm, useFieldArray, SubmitHandler,  } from "react-hook-form";

import {
    JournalEntry,
    JournalTag,
    EntryForm,
    EntryTagForm,
    get_date,
    blank_form,
    entry_to_form,
    retrieve_entry,
    create_entry,
    update_entry,
    delete_entry
} from "./journal";

interface EntrySecProps {
    title: JSX.Element,
    children?: JSX.Element[] | JSX.Element
}

const EntrySec = ({title, children}: EntrySecProps) => {
    return <>
        <div>{title}</div>
        <div>{children}</div>
    </>
};

interface EntrySecTitleProps {
    title: String
}

const EntrySecTitle = ({title}: EntrySecTitleProps) => {
    return <span className="text-right">{title}</span>
};

const Entry = () => {
    const { entry_date } = useParams();
    const navigate = useNavigate();

    const form = useForm<EntryForm>({
        defaultValues: blank_form()
    });
    const tags = useFieldArray<EntryForm, "tags">({
        control: form.control,
        name: "tags"
    });

    let [loading, setLoading] = useState(true);

    const onSubmit: SubmitHandler<EntryForm> = (data, event) => {
        console.log(data);

        if (entry_date == null || entry_date == "new") {
            create_entry(data).then(result => {
                console.log("created entry:", result);

                navigate(`/entries/${result.date}`);

                form.reset(entry_to_form(result));
            }).catch(err => {
                console.error("failed to create entry:", err);
            })
        } else {
            update_entry(entry_date, data).then(result => {
                console.log("updated entry:", result);
            }).catch(err => {
                console.error("failed to update entry:", err);
            });
        }
    };

    useEffect(() => {
        console.log("entry date:", entry_date);

        if (entry_date == null || entry_date == "new") {
            console.log("resetting to blank");

            form.reset(blank_form());

            return;
        }

        retrieve_entry(entry_date).then(entry => {
            console.log("resetting to entry:", entry);

            form.reset(entry_to_form(entry));
        }).catch(err => {
            console.error("failed to retrieve entry:", err);
        });
    }, [entry_date]);

    return <form onSubmit={form.handleSubmit(onSubmit)}>
        <div
            className=""
            style={{
                display: "grid",
                gridTemplateColumns: "10rem auto"
            }}
        >
            <EntrySec title={<EntrySecTitle title="Date"/>}>
                <input type="date" {...form.register("date")}/>
            </EntrySec>
            <EntrySec title={<EntrySecTitle title="Title"/>}>
                <input type="text" {...form.register("title")}/>
            </EntrySec>
            <EntrySec title={<EntrySecTitle title="Contents"/>}>
                <textarea {...form.register("contents")}/>
            </EntrySec>
            <EntrySec title={<EntrySecTitle title="Tags"/>}>
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
            </EntrySec>
        </div>
        <div>
            <button type="submit">Save</button>
            {entry_date != null && entry_date !== "new" ?
                <button type="button" onClick={() => {
                    delete_entry(entry_date).then(result => {
                        console.log("deleted entry:", result);

                        navigate("/entries");
                    }).catch(err => {
                        console.error("failed to delete journal entry");
                    });
                }}>Delete</button>
                :
                null
            }
        </div>
    </form>
};

export default Entry;
