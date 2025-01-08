
import replace from "@rollup/plugin-replace";

function manual_chunks(id, {getModuleInfo, getModuleIds}) {
    if (id.includes("node_modules")) {
        return "vendor";
    }
}

function onwarn(warning, warn) {
    if (warning.code === "MODULE_LEVEL_DIRECTIVE") {
        return;
    }

    warn(warning);
}

export default {
    input: {
        index: "./frontend/src/index.tsx",
    },
    onwarn,
    output: {
        dir: "./frontend/assets/",
        format: "es",
        sourcemap: true,
        manualChunks: manual_chunks,
    },
    resolve: {
        tsconfigFilename: "./tsconfig.json",
    },
    plugins: [
        replace({
            values: {
                "process.env.NODE_ENV": JSON.stringify("development"),
            },
            preventAssignment: true,
        }),
    ]
}
