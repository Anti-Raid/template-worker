import { decode, type KhronosValue, type RawKhronosValue } from "./msyscall/khronosvalue";

export type DispatchResult = {
    id: string,
    type: "ok",
    value: RawKhronosValue
} | {
    id: string,
    type: "err",
    value: KhronosValue
}

export type Event = {
    type: "formset_reorder", 
    __tloop_template_id: string, // the template id to dispatch to, used by tloop to selectively dispatch
    id: string,
    list: string[]
} | {
    type: "form_action",
    __tloop_template_id: string, // the template id to dispatch to, used by tloop to selectively dispatch
    action_button_id: string, 
    formset_id: string,
    form_id: string, 
    form_data?: Record<string, any>
} | {
    type: "fetch_page",
}

const COMPONENT_TYPES = [
    "Section", "FormSet", "DisplayElement"
] as const

const DISPLAY_ELEMENT_TYPES = [
    "Header", "Paragraph", "Error"
] as const

export type DisplayElement = {
    /** a header */
    type: "Header",
    text: string
} | {
    /** a paragraph */
    type: "Paragraph",
    text: string
} | {
    /** an error */
    type: "Error",
    error: string
}

/** A base component */
export type Component = {
    /** A raw text block */
    type: "DisplayElement",
    element: DisplayElement
} | {
    /** A section block */
    type: "Section",
    id: string,
    title: string,
    description: string,
    entries: Component[]
} | {
    /* A set of forms */
    type: "FormSet",
    /** formset id */
    id: string, 
    /** if set, a reorder event will be sent with the new list of ids */
    reorderable: boolean,
    /** Form elements */
    forms: FormElement[],
    /** Form actions */
    actions: FormAction[]
}

const RAW_FORM_ACTION_TYPES = [
    "Button.Event"
] as const

/** Form Actions */
export type FormAction = {
    type: "Button.Event",
    id: string,
    text: string,
    style: "Primary" | "Secondary" | "Danger",
    /**  if set to true, will send the entire form state */
    send_form: boolean 
}

const FORM_ELEMENT_TYPES = [
    "DisplayElement", "Text", "Array.Text", "Array.Select.Text", "Number", "Select.Text",
    "Boolean"
] as const

export type FormElement = {
    type: "DisplayElement",
    element: DisplayElement
} | {
    type: "Text",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
} | {
    type: "Array.Text",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
} | {
    type: "Array.Select.Text",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    choices: {label: string, value: string}[],
} | {
    type: "Number",
    id: string,
    label: string,
    description?: string,
    placeholder?: string,
    disabled?: boolean,
} | {
    type: "Select.Text",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
    placeholder?: string,
    choices: {label: string, value: string}[],
} | {
    type: "Boolean",
    id: string,
    label: string,
    description?: string,
    disabled?: boolean,
}

export type FormData = {
    /*form id*/
    id: string,
    /**form title*/
    title: string,
    /*form data*/
    data: Record<string, any>,
}

/** Unprocessed form data sent directly from the server */
type RawFormData = {
    /*form id*/
    id: string,
    /**form title*/
    title: string,
    /*form data*/
    data: Map<string, RawKhronosValue>,
}

export type Page = {
    components: Component[],
    formdata: Record<string, FormData[]>,
}

const _isOneOf = <T extends string>(value: string, allowedList: readonly T[]): value is T =>{
  return (allowedList as readonly string[]).includes(value);
}

const assertOneOf = <T extends string>(value: string, allowedList: readonly T[]): T => {
  if (_isOneOf(value, allowedList)) {
    return value;
  }
  throw new Error(`Invalid value: "${value}". Expected one of: ${allowedList.join(', ')}`);
}

const getTypeName = (value: RawKhronosValue): string => {
    if (typeof value === "string") return value
    return Object.keys(value)[0] || "Unknown"
}

const assertOptional = <T>(value: RawKhronosValue | undefined, fn: (value: RawKhronosValue, ty?: string) => T, ty: string = "optional"): T | undefined => {
    if (value === undefined || "Nil" in value || "Null" in value) return undefined
    if(ty) return fn(value, ty)
    else return fn(value)
}

const assertString = (value: RawKhronosValue, ty: string = "string"): string => {
    if (!("Text" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.Text
}

const assertNumber = (value: RawKhronosValue, ty: string = "number"): number => {
    if("Float" in value) return value.Float
    else if("Integer" in value) return value.Integer
    throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
}

const assertBoolean = (value: RawKhronosValue, ty: string = "boolean"): boolean => {
    if (!("Boolean" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.Boolean
}

const assertList = (value: RawKhronosValue, ty: string = "list"): RawKhronosValue[] => {
    if (!("List" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    return value.List
}

const assertMap = (value: RawKhronosValue, ty = "Map with string keys"): Map<string, RawKhronosValue> => {
    if (!("Map" in value || "StrMap" in value)) throw new Error(`Got ${getTypeName(value)} when ${ty} expected`)
    let mp: Map<string, RawKhronosValue> = new Map()

    // handle strmap
    if("StrMap" in value) {
        for(const [key, val] of Object.entries(value.StrMap)) {
            mp.set(key, val)
        }
        return mp
    }

    // Map
    for(const [key, val] of value.Map) {
        let k = assertString(key, "Map with string keys")
        mp.set(k, val)
    }
    return mp
}

const mapGet = (map: Map<string, RawKhronosValue>, prop: string, inprop: string = "map"): RawKhronosValue => {
    let p = map.get(prop)
    if(!p) throw new Error(`\`${prop}\` not found in ${inprop}`)
    return p
}

const mapGetOpt = (map: Map<string, RawKhronosValue>, prop: string): RawKhronosValue | undefined => {
    let p = map.get(prop)
    if(!p || "Nil" in p || "Null" in p) return undefined
    return p
}

/**
 * Given a value from a event, returns the DispatchResult created by Luau
 */
export const toDispatchResults = (value: RawKhronosValue): DispatchResult[] => {
    const l = assertList(value, "list of DispatchResult's")
    const results: DispatchResult[] = []
    for(const result of l) {
        const resultMap = assertMap(result)
        const id = assertString(mapGet(resultMap, "id"), "id")
        const type = assertOneOf(assertString(mapGet(resultMap, "type"), "type"), ["ok", "err"])
        const value = mapGet(resultMap, "value")
        if(type === "ok") {
            results.push({ id, type, value })
        } else {
            const decodedVal = decode(value)
            results.push({ id, type: "err", value: decodedVal })
        }
    }
    return results
}

/** Helper method to unpack a RawFormData w/ set of settings v2 FormElements into a processed FormData */
const settingsUnwrapRawFormData = (elements: FormElement[], formdata: RawFormData): FormData => {
    const data: Record<string, any> = {}
    for(const rawel of elements) {
        if(rawel.type === "DisplayElement") continue // display elements dont have form data
        const rawVal = mapGetOpt(formdata.data, rawel.id);

        switch (rawel.type) {
            case "Text":
            case "Select.Text":
                data[rawel.id] = rawVal !== undefined ? assertString(rawVal) : ""
                break
            case "Number":
                data[rawel.id] = rawVal !== undefined ? assertNumber(rawVal) : 0
                break
            case "Boolean":
                data[rawel.id] = rawVal !== undefined ? assertBoolean(rawVal) : false
                break
            case "Array.Text":
            case "Array.Select.Text":
                data[rawel.id] = rawVal !== undefined ? assertList(rawVal).map(v => assertString(v, "string in string array")) : []
                break
        }
    }

    return { id: formdata.id, title: formdata.title, data }
}

/**
 * Given the event response (DispatchResult.value for type="ok"), parse the setting
 */
export const dispatchResultToSetting = (value: RawKhronosValue): Page => {
    const MAX_DEPTH: number = 10;
    
    const page = assertMap(value, "Page")

    // Extract out formdata first
    const rawFormDataObjMap = assertMap(mapGet(page, "formdata"), "Page#formdata")
    const rawFormDataObj: Map<string, RawFormData[]> = new Map()
    for(const [key, formVals] of rawFormDataObjMap) {
        rawFormDataObj.set(key, assertList(formVals).map((fv, idx) => {
           const map = assertMap(fv, `FormData map at idx ${idx} of Page#formdata`)
           const id = assertString(mapGet(map, "id"), "id")
           const title = assertString(mapGet(map, "title"), "title")
           const data = assertMap(mapGet(map, "data"), "data")
           return {id, title, data}
        }))
    }

    // Create storage spot for processed formdatas
    const formData: Record<string, FormData[]> = {}

    const expandDisplayElement = (map: Map<string, RawKhronosValue>): DisplayElement => {
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), DISPLAY_ELEMENT_TYPES)
        switch (type) {
            case "Header":
            case "Paragraph":
                const text = assertString(mapGet(map, "text"), "text")
                return { type, text }
            case "Error":
                const error = assertString(mapGet(map, "error"), "error")
                return { type, error }
        }
    }

    const expandFormAction = (map: Map<string, RawKhronosValue>): FormAction => {
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), RAW_FORM_ACTION_TYPES)
        switch (type) {
        case "Button.Event":
            const ebid = assertString(mapGet(map, "id"), "id")
            const ebtext = assertString(mapGet(map, "text"), "text")
            const ebstyle = assertOneOf(assertString(mapGet(map, "style"), "style"), ["Primary", "Secondary", "Danger"])
            const ebsendform = assertBoolean(mapGet(map, "send_form"), "send_form")
            return { type, id: ebid, text: ebtext, style: ebstyle, send_form: ebsendform }
        }
    }

    const expandRawFormElement = (map: Map<string, RawKhronosValue>): FormElement => {
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), FORM_ELEMENT_TYPES)
        switch (type) {
            case "DisplayElement":
                const delem = expandDisplayElement(assertMap(mapGet(map, "element"), "element"))
                return { type, element: delem }
            case "Text":
            case "Number": // all of these share the same base type (for now)
               const tid = assertString(mapGet(map, "id"), "id")
               const tlabel = assertString(mapGet(map, "label"), "label")
               const tdesc = assertOptional(mapGetOpt(map, "description"), assertString)
               const tph = assertOptional(mapGetOpt(map, "placeholder"), assertString)
               const tdisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
               return { type, id: tid, label: tlabel, description: tdesc, placeholder: tph, disabled: tdisabled }
            case "Array.Text":
                const aid = assertString(mapGet(map, "id"), "id")
                const alabel = assertString(mapGet(map, "label"), "label")
                const adesc = assertOptional(mapGetOpt(map, "description"), assertString)
                const adisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
                return { type, id: aid, label: alabel, description: adesc, disabled: adisabled }
            case "Array.Select.Text":
                const said = assertString(mapGet(map, "id"), "id")
                const salabel = assertString(mapGet(map, "label"), "label")
                const sadesc = assertOptional(mapGetOpt(map, "description"), assertString)
                const sadisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
                const sachoices = assertList(mapGet(map, "choices"), "choices").map((entry, idx) => {
                    const cmap = assertMap(entry, `choices map at idx ${idx} for Select.Array.Text with id ${idx} [${sid}]`) // expand the entry into choices
                    const clabel = assertString(mapGet(cmap, "label"), "label")
                    const cvalue = assertString(mapGet(cmap, "value"), "value")
                    return { label: clabel, value: cvalue }
                })
                return { type, id: said, label: salabel, description: sadesc, disabled: sadisabled, choices: sachoices }
            case "Select.Text":
                const sid = assertString(mapGet(map, "id"), "id")
                const slabel = assertString(mapGet(map, "label"), "label")
                const sph = assertOptional(mapGetOpt(map, "placeholder"), assertString)
                const sdesc = assertOptional(mapGetOpt(map, "description"), assertString)
                const sdisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
                const schoices = assertList(mapGet(map, "choices"), "choices").map((entry, idx) => {
                    const cmap = assertMap(entry, `choices map at idx ${idx} for Select.Text with id ${idx} [${sid}]`) // expand the entry into choices
                    const clabel = assertString(mapGet(cmap, "label"), "label")
                    const cvalue = assertString(mapGet(cmap, "value"), "value")
                    return { label: clabel, value: cvalue }
                })
                return { type, id: sid, label: slabel, description: sdesc, placeholder: sph, disabled: sdisabled, choices: schoices }
            case "Boolean":
               const bid = assertString(mapGet(map, "id"), "id")
               const blabel = assertString(mapGet(map, "label"), "label")
               const bdesc = assertOptional(mapGetOpt(map, "description"), assertString)
               const bdisabled = assertOptional(mapGetOpt(map, "disabled"), assertBoolean)
               return { type, id: bid, label: blabel, description: bdesc, disabled: bdisabled }
        }
    }

    const expandComponent = (map: Map<string, RawKhronosValue>, depth: number): Component[] => {
        if(depth > MAX_DEPTH) throw new Error(`Spec violation: above max depth of ${MAX_DEPTH} in expandComponent`)
        const type = assertOneOf(assertString(mapGet(map, "type"), "type"), COMPONENT_TYPES)
        switch (type) {
            case "DisplayElement":
                const delem = expandDisplayElement(assertMap(mapGet(map, "element"), "element"))
                return [{ type, element: delem }]
            case "Section":
                const sid = assertString(mapGet(map, "id"), "id")
                const stitle = assertString(mapGet(map, "title"), "title")
                const sdesc = assertString(mapGet(map, "description"), "description")
                const sentries = assertList(mapGet(map, "entries"), "entries").flatMap((entry, idx) => {
                    return expandComponent(assertMap(entry, `component map at idx ${idx} for section with id ${sid}`), depth+1) // recursively expand the entry into more components
                })
                return [{ type: "Section", id: sid, title: stitle, description: sdesc, entries: sentries }]
            case "FormSet":
                const fsid = assertString(mapGet(map, "id"), "id")
                const fsreorderable = assertBoolean(mapGet(map, "reorderable"), "reorderable")
                const baseForm = assertList(mapGet(map, "form"), "form").map((formListElem, idx) => {
                    return expandRawFormElement(assertMap(formListElem, `FormElement at idx ${idx} of FormSet \`${fsid}\``))
                })
                const formActions = assertList(mapGet(map, "actions"), "actions").map((actionElem, idx) => {
                    return expandFormAction(assertMap(actionElem, `FormAction at idx ${idx} of FormSet \`${fsid}\``))
                })

                // process raw form datas
                const procforms: FormData[] = []
                for(const form of rawFormDataObj.get(fsid) ?? []) {
                    procforms.push(settingsUnwrapRawFormData(baseForm, form))
                }
                formData[fsid] = procforms

                return [{ type: "FormSet", id: fsid, forms: baseForm, reorderable: fsreorderable, actions: formActions }]
        }
    }

    // extract components out
    const comps = assertList(mapGet(page, "components"), "Page#components").flatMap((x, idx) => {
        return expandComponent(assertMap(x, `Component at idx ${idx} of Page#components`), 0)
    })

    return { components: comps, formdata: formData }
}