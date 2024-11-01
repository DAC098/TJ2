import { useState, useEffect, JSX } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";

import TagEntry from "./TagEntry";
import FileEntry from "./FileEntry";
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
    delete_entry,
    upload_data,
} from "../journal";

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
    return <div className="text-right w-full">{title}</div>
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

    const create_and_upload = async (entry: EntryForm): Promise<[boolean, JournalEntry]> => {
        let mapped = {};
        let promise = create_entry(entry);

        if (entry.files.length !== 0) {
            for (let file of entry.files) {
                if (file.type == "server") {
                    continue;
                }

                mapped[file.key] = file;
            }
        }

        let failed = false;
        let result = await promise;

        console.log("created entry:", result);

        if (result.files.length !== 0 ) {
            let promises = [];

            for (let file_entry of result.files) {
                let ref = mapped[file_entry.attached.key];

                if (ref == null) {
                    console.log("file key not known to client", file_entry.attached.key);

                    continue;
                }

                let prom = upload_data(result.date, file_entry, ref)
                    .then(success => {
                        if (success) {
                            console.log("uploaded file", file_entry.id);
                        } else {
                            failed = true;
                        }
                    });

                promises.push(prom);
            }

            let prom_results = await Promise.allSettled(promises);

            for (let prom of prom_results) {
                switch (prom.status) {
                case "fulfilled":
                    console.log("file uploaded", prom.value);

                    break;
                case "rejected":
                    console.log("file failed", prom.reason);

                    failed = true;

                    break;
                }
            }
        }

        return [failed, result];
    };

    const onSubmit: SubmitHandler<EntryForm> = async (data, event) => {
        console.log(data);

        if (entry_date == null || entry_date == "new") {
            try {
                let [failed, result] = await create_and_upload(data);

                if (!failed) {
                    form.reset(entry_to_form(result));

                    navigate(`/entries/${result.date}`);
                }
            } catch(err) {
                console.error("failed to create entry:", err);
            }
        } else {
            try {
                let result = await update_entry(entry_date, data);

                console.log("updated entry:", result);
            } catch(err) {
                console.error("failed to update entry:", err);
            }
        }
    };

    useEffect(() => {
        console.log("entry date:", entry_date);

        let form_date = form.getValues("date");

        if (form_date == entry_date) {
            console.log("form date is same as entry, assume entry was just created");
        }

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

    return <FormProvider<EntryForm> {...form} children={
        <form onSubmit={form.handleSubmit(onSubmit)}>
            <div className="top-0 sticky">
                <input type="date" {...form.register("date")}/>
                <button type="submit">Save</button>
                {entry_date != null && entry_date !== "new" ?
                    <button type="button" onClick={() => {
                        delete_entry(entry_date).then(() => {
                            console.log("deleted entry");

                            navigate("/entries");
                        }).catch(err => {
                            console.error("failed to delete journal entry:", err);
                        });
                    }}>Delete</button>
                    :
                    null
                }
            </div>
            <div
                className=""
                style={{
                    display: "grid",
                    gridTemplateColumns: "10rem auto"
                }}
            >
                <EntrySec title={<EntrySecTitle title="Title"/>}>
                    <input type="text" {...form.register("title")}/>
                </EntrySec>
                <EntrySec title={<EntrySecTitle title="Contents"/>}>
                    <textarea {...form.register("contents")}/>
                </EntrySec>
                <EntrySec title={<EntrySecTitle title="Files"/>}>
                    <FileEntry entry_date={entry_date}/>
                </EntrySec>
                <EntrySec title={<EntrySecTitle title="Tags"/>}>
                    <TagEntry/>
                </EntrySec>
            </div>
        </form>
    }/>
};

export default Entry;
