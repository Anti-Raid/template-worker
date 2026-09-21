import { Anima, implRvm, Table, BuiltinFunction, Cons, common } from 'animalang';

export type AbstractClosure = common.AbstractClosure;

export type FieldOptions = {
    placeholder?: string;
    description?: string;
    disabled?: boolean;
    [key: string]: any;
};

export function parseId(val: any): string {
    if (typeof val === 'symbol') {
        return val.description || String(val);
    }
    return String(val ?? "");
}

export function getOpt(opts: any, key: string, fallback?: any): any {
    if (opts == null) return fallback;
    if (opts instanceof Table) {
        const val = opts.get(key) ?? opts.get(Symbol.for(key));
        if (val !== undefined) return val;
        if (key === "description" || key === "desc") {
            return opts.get("desc") ?? opts.get(Symbol.for("desc")) ?? opts.get("description") ?? opts.get(Symbol.for("description")) ?? fallback;
        }
        return fallback;
    }
    throw new Error(`Expected options to be a Table, but received ${typeof opts} (${opts})`);
}

export function parseFieldOpts(rawOpts: any): FieldOptions {
    if (rawOpts == null) return {};
    if (!(rawOpts instanceof Table)) throw new Error(`Expected options to be a Table, but received ${typeof rawOpts}`);
    const opts: FieldOptions = {};
    const placeholder = getOpt(rawOpts, "placeholder"), desc = getOpt(rawOpts, "description"), dis = getOpt(rawOpts, "disabled");
    if (placeholder !== undefined) opts.placeholder = String(placeholder);
    if (desc !== undefined) opts.description = String(desc);
    if (dis !== undefined) opts.disabled = Boolean(dis);
    return opts;
}

export function parseChoices(rawChoices: any): { label: string; value: string }[] {
    if (!rawChoices) return [];
    return unpackList(rawChoices).filter(Boolean).map(item => {
        if (item instanceof Table) {
            const label = getOpt(item, "label") ?? getOpt(item, "value") ?? "";
            const value = getOpt(item, "value") ?? getOpt(item, "label") ?? "";
            return { label: String(label), value: String(value) };
        }
        if (item instanceof Cons) {
            const car = parseId(item.car);
            const cdr = item.cdr instanceof Cons ? parseId(item.cdr.car) : (item.cdr !== null ? parseId(item.cdr) : car);
            return { value: car, label: cdr };
        }
        if (Array.isArray(item)) {
            const val = parseId(item[0]);
            return { value: val, label: item.length > 1 ? parseId(item[1]) : val };
        }
        if (typeof item === "string" || typeof item === "symbol" || typeof item === "number") {
            const str = parseId(item);
            return { value: str, label: str };
        }
        throw new Error(`Invalid choice item in select: expected Table, Cons pair, Vector, or string, but received ${typeof item}`);
    });
}

export function unpackList(raw: any): any[] {
    if (raw == null) return [];
    return raw instanceof Cons ? raw.toArray() : (Array.isArray(raw) ? raw : [raw]);
}

// --- Component Class Hierarchy ---

export abstract class BaseComponent {
    public abstract readonly compType: string;
}

export abstract class BaseAction extends BaseComponent {
    public readonly compType = "Action";
    public abstract readonly actionType: string;
    public formset: FormSetComponent | null = null;
    public session: FormSession | null = null;
    constructor(public id: string, public sendForm: boolean = true) {
        super();
    }
}

export class UIActionButton extends BaseAction {
    public readonly actionType = "Button";
    constructor(id: string, public text: string, public style: "Primary" | "Secondary" | "Danger" = "Primary", sendForm: boolean = true) {
        super(id, sendForm);
    }
}

export class UIDisplayElement extends BaseComponent {
    public readonly compType = "Display";
    constructor(public displayType: "Error" | "Header" | "Paragraph", public text: string) {
        super();
    }
}

export class SectionComponent extends BaseComponent {
    public readonly compType = "Section";
    constructor(public id: string, public title: string, public description: string, public entries: BaseComponent[] = []) {
        super();
    }
}

export class FormSetComponent extends BaseComponent {
    public readonly compType = "FormSet";
    constructor(public id: string, public reorderable: boolean = false, public deps: string[] = [], public templateFn: AbstractClosure | null = null) {
        super();
    }

    public isDep(id: string): boolean {
        return this.deps.includes("*") || this.deps.includes(id);
    }

    public createSession(engine: Anima, initialData?: Record<string, any> | Table, onChange?: FormChangeListener): FormSession {
        return new FormSession(engine, this, initialData, onChange);
    }

    public instantiate(engine: Anima, rowData: Record<string, any> | Table): { forms: BaseComponent[]; actions: BaseAction[] } {
        if (!this.templateFn) return { forms: [], actions: [] };
        const tableData = rowData instanceof Table ? rowData : Table.fromObject(rowData, true);
        const partitioned = partitionFormElements(engine.evaluateClosure(this.templateFn, [tableData]));
        for (const f of partitioned.forms) if (f instanceof BaseInput) f.formset = this;
        for (const a of partitioned.actions) a.formset = this;
        return partitioned;
    }
}

export abstract class BaseInput<T = any> extends BaseComponent {
    public readonly compType = "Input";
    public abstract readonly inputType: string;
    public formset: FormSetComponent | null = null;
    public session: FormSession | null = null;

    constructor(public id: string, public label: string, public value: T, public opts: FieldOptions = {}) {
        super();
    }

    public setValue(newValue: T): boolean {
        this.value = newValue;
        return this.session ? this.session.setValue(this.id, newValue) : false;
    }
}

export class UITextInput extends BaseInput<string> { public readonly inputType = "Text"; }
export class UIRoleInput extends BaseInput<string> { public readonly inputType = "Role"; }
export class UIChannelInput extends BaseInput<string> { public readonly inputType = "Channel"; }
export class UIMemberInput extends BaseInput<string> { public readonly inputType = "Member"; }
export class UINumberInput extends BaseInput<number> { public readonly inputType = "Number"; }
export class UIBooleanInput extends BaseInput<boolean> { public readonly inputType = "Boolean"; }
export class UIDateTimeInput extends BaseInput<string> { public readonly inputType = "DateTime"; }
export class UIMultiTextInput extends BaseInput<string[]> { public readonly inputType = "MultiText"; }

export class UISelectInput extends BaseInput<string> {
    public readonly inputType = "Select";
    constructor(id: string, label: string, value: string, public choices: { label: string; value: string }[], opts: FieldOptions = {}) {
        super(id, label, value, opts);
    }
}

export class PageComponent {
    constructor(public components: BaseComponent[] = [], public formdata: Record<string, any[]> = {}) {}
}

export function partitionFormElements(rawItems: any): { forms: BaseComponent[]; actions: BaseAction[] } {
    const forms: BaseComponent[] = [], actions: BaseAction[] = [];
    for (const item of unpackList(rawItems)) {
        if (!item) continue;
        if (item instanceof BaseAction) actions.push(item);
        else if (item instanceof BaseComponent) forms.push(item);
        else throw new Error(`Expected form element to be an instance of BaseComponent, but received: ${item}`);
    }
    return { forms, actions };
}

// --- Generic Form State Management (Framework-agnostic) ---

export type FormChangeMeta = {
    changedId: string;
    value: any;
    structural: boolean;
};

export type FormChangeListener = (session: FormSession, meta: FormChangeMeta) => void;

export class FormSession {
    public forms: BaseComponent[] = [];
    public actions: BaseAction[] = [];
    public data: Table;
    #listeners: Set<FormChangeListener> = new Set();
    #lastDepValues: Map<string, any> = new Map();

    constructor(
        public engine: Anima,
        public formset: FormSetComponent,
        initialData: Record<string, any> | Table = new Table(),
        onChange?: FormChangeListener
    ) {
        this.data = initialData instanceof Table ? initialData : Table.fromObject(initialData, true);
        if (onChange) this.#listeners.add(onChange);
        this.reinstantiate();
    }

    public subscribe(listener: FormChangeListener): () => void {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }

    public setValue(id: string, value: any): boolean {
        const isDep = this.formset.isDep(id);
        if (!isDep || !this.#hasDepChanged(id, value)) {
            this.data.set(id, value);
            this.#updateInMemoryField(id, value);
            this.#notifyListeners(id, value, false);
            return false;
        }

        this.data.set(id, value);
        this.reinstantiate();
        this.#notifyListeners(id, value, true);
        return true;
    }

    public getValue<T = any>(id: string, fallback?: T): T | undefined {
        return this.data.has(id) ? this.data.get(id) : fallback;
    }

    public reinstantiate(): void {
        const fresh = this.formset.instantiate(this.engine, this.data);
        const reconciled = this.#reconcileComponents(fresh.forms, fresh.actions);
        this.forms = reconciled.forms;
        this.actions = reconciled.actions;
        this.#snapshotDeps();
    }

    #updateInMemoryField(id: string, value: any): void {
        for (const form of this.forms) {
            if (form instanceof BaseInput && form.id === id) {
                form.value = value;
                break;
            }
        }
    }

    #snapshotDeps(): void {
        this.#lastDepValues.clear();
        for (const dep of this.formset.deps) {
            if (dep !== "*") this.#lastDepValues.set(dep, this.data.get(dep));
        }
    }

    #hasDepChanged(id: string, value: any): boolean {
        if (this.formset.deps.includes("*") || !this.#lastDepValues.has(id)) return true;
        return this.#lastDepValues.get(id) !== value;
    }

    #reconcileComponents(freshForms: BaseComponent[], freshActions: BaseAction[]): { forms: BaseComponent[]; actions: BaseAction[] } {
        const existingInputs = new Map<string, BaseInput>();
        const existingActions = new Map<string, BaseAction>();
        for (const form of this.forms) if (form instanceof BaseInput) existingInputs.set(form.id, form);
        for (const act of this.actions) existingActions.set(act.id, act);

        const patchInput = (fresh: BaseInput, existing: BaseInput) => {
            existing.value = fresh.value;
            existing.label = fresh.label;
            existing.opts = fresh.opts;
            if (existing instanceof UISelectInput && fresh instanceof UISelectInput) existing.choices = fresh.choices;
            return existing;
        };

        const patchAction = (fresh: BaseAction, existing: BaseAction) => {
            if (existing instanceof UIActionButton && fresh instanceof UIActionButton) {
                existing.text = fresh.text;
                existing.style = fresh.style;
                existing.sendForm = fresh.sendForm;
            }
            return existing;
        };

        const reconciledForms = freshForms.map(fresh => {
            if (fresh instanceof BaseInput) {
                const existing = existingInputs.get(fresh.id);
                const target = (existing && existing.constructor === fresh.constructor) ? patchInput(fresh, existing) : fresh;
                target.session = this;
                target.formset = this.formset;
                return target;
            }
            return fresh;
        });

        const reconciledActions = freshActions.map(fresh => {
            const existing = existingActions.get(fresh.id);
            const target = (existing && existing.constructor === fresh.constructor) ? patchAction(fresh, existing) : fresh;
            target.session = this;
            target.formset = this.formset;
            return target;
        });

        return { forms: reconciledForms, actions: reconciledActions };
    }

    #notifyListeners(changedId: string, value: any, structural: boolean): void {
        const meta: FormChangeMeta = { changedId, value, structural };
        for (const listener of this.#listeners) {
            try {
                listener(this, meta);
            } catch (err) {
                console.error(`FormChangeListener error on field '${changedId}':`, err);
            }
        }
    }
}

function collectComponents(regs: readonly any[], start: number, count: number): BaseComponent[] {
    const list: BaseComponent[] = [];
    for (let i = 0; i < count; i++) {
        for (const item of unpackList(regs[start + i])) {
            if (item instanceof BaseComponent) list.push(item);
        }
    }
    return list;
}

function makeInputBuiltin<T>(
    name: string,
    Ctor: new (id: string, label: string, value: T, opts?: FieldOptions) => BaseInput<T>,
    transform: (raw: any) => T = (v) => v
): BuiltinFunction {
    return new BuiltinFunction(Symbol.for(name), (regs, startReg, nargs) => {
        if (nargs < 3) throw new Error(`${name} requires 3 arguments: (${name} id label value [opts])`);
        return new Ctor(
            parseId(regs[startReg]),
            String(regs[startReg + 1] ?? ""),
            transform(regs[startReg + 2]),
            nargs >= 4 ? parseFieldOpts(regs[startReg + 3]) : {}
        );
    });
}

export const SV2_BUILTINS: BuiltinFunction[] = [
    new BuiltinFunction(Symbol.for("page"), (regs, start, n) => new PageComponent(collectComponents(regs, start, n))),

    new BuiltinFunction(Symbol.for("section"), (regs, start, n) => {
        if (n < 3) throw new Error("section requires at least 3 arguments: (section id title desc ...entries)");
        return new SectionComponent(parseId(regs[start]), String(regs[start + 1] ?? ""), String(regs[start + 2] ?? ""), collectComponents(regs, start + 3, n - 3));
    }),

    new BuiltinFunction(Symbol.for("formset"), (regs, start, n) => {
        if (n < 2) throw new Error("formset requires at least 2 arguments: (formset id [opts] template-fn)");
        const id = parseId(regs[start]);
        const opts = n === 2 ? null : regs[start + 1];
        const templateFn = n === 2 ? regs[start + 1] : regs[start + 2];
        const reorderable = Boolean(getOpt(opts, "reorderable", false));
        const deps = unpackList(getOpt(opts, "deps", [])).map(d => parseId(d));
        return new FormSetComponent(id, reorderable, deps, templateFn);
    }),

    new BuiltinFunction(Symbol.for("ui/display"), (regs, start, n) => {
        if (n < 2) throw new Error("ui/display requires 2 arguments: (ui/display type text)");
        const raw = parseId(regs[start]).toLowerCase();
        const displayType: "Error" | "Header" | "Paragraph" = raw === "error" ? "Error" : raw === "header" ? "Header" : "Paragraph";
        return new UIDisplayElement(displayType, String(regs[start + 1] ?? ""));
    }),

    makeInputBuiltin("ui/text", UITextInput, v => String(v ?? "")),
    makeInputBuiltin("ui/number", UINumberInput, v => Number(v ?? 0)),
    makeInputBuiltin("ui/role", UIRoleInput, v => String(v ?? "")),
    makeInputBuiltin("ui/channel", UIChannelInput, v => String(v ?? "")),
    makeInputBuiltin("ui/member", UIMemberInput, v => String(v ?? "")),
    makeInputBuiltin("ui/boolean", UIBooleanInput, v => Boolean(v)),
    makeInputBuiltin("ui/datetime", UIDateTimeInput, v => String(v ?? "")),
    makeInputBuiltin("ui/multi-text", UIMultiTextInput, v => unpackList(v).map(x => String(x))),

    new BuiltinFunction(Symbol.for("ui/select"), (regs, start, n) => {
        if (n < 4) throw new Error("ui/select requires 4 arguments: (ui/select id label value choices [opts])");
        return new UISelectInput(
            parseId(regs[start]),
            String(regs[start + 1] ?? ""),
            String(regs[start + 2] ?? ""),
            parseChoices(regs[start + 3]),
            n >= 5 ? parseFieldOpts(regs[start + 4]) : {}
        );
    }),

    new BuiltinFunction(Symbol.for("ui/action-button"), (regs, start, n) => {
        if (n < 2) throw new Error("ui/action-button requires at least 2 arguments: (ui/action-button id text [opts])");
        const id = parseId(regs[start]);
        const text = String(regs[start + 1] ?? "");
        const opts = n >= 3 ? regs[start + 2] : null;
        const styleRaw = parseId(getOpt(opts, "style", "Primary")).toLowerCase();
        const style: "Primary" | "Secondary" | "Danger" =
            styleRaw === "danger" ? "Danger" : styleRaw === "secondary" ? "Secondary" : "Primary";
        const sendForm = Boolean(getOpt(opts, "send-form", getOpt(opts, "send_form", true)));
        return new UIActionButton(id, text, style, sendForm);
    })
];

export function registerSV2Builtins(scope: common.Globals): void {
    for (const b of SV2_BUILTINS) {
        scope.set(b.name, b);
    }
}

// --- SettingsAnima Environment ---

export class SettingsAnima {
    #engine: Anima;

    constructor() {
        this.#engine = new Anima(implRvm);
        registerSV2Builtins(this.#engine.scope);
    }

    public get engine(): Anima {
        return this.#engine;
    }

    public evaluatePage(script: string): PageComponent {
        const bc = this.#engine.compileRaw(script);
        const res = this.#engine.evaluateRaw(bc);
        if (res instanceof PageComponent) return res;
        if (Array.isArray(res)) return new PageComponent(res);
        if (res instanceof BaseComponent) return new PageComponent([res]);
        throw new Error(`Expected script to evaluate to PageComponent, but received: ${res}`);
    }

    public evaluateForm(
        formset: FormSetComponent,
        rowData: Record<string, any> | Table
    ): { forms: BaseComponent[]; actions: BaseAction[] } {
        return formset.instantiate(this.#engine, rowData);
    }

    public createFormSession(
        formset: FormSetComponent,
        initialData?: Record<string, any> | Table,
        onChange?: FormChangeListener
    ): FormSession {
        return new FormSession(this.#engine, formset, initialData, onChange);
    }
}
