import { describe, it, expect, vi } from 'vitest';
import { Table } from 'animalang';
import {
    SettingsAnima,
    FormSetComponent,
    BaseInput,
    UITextInput,
    UINumberInput,
    UISelectInput,
    UIActionButton,
    PageComponent,
    SectionComponent,
    UIDisplayElement
} from './sv2.anima';

describe('SV2 Anima runtime and FormSession', () => {
    it('evaluates a full page with section, display, and formset', () => {
        const anima = new SettingsAnima();
        const script = `
            (page
                (ui/display "header" "Server Configuration")
                (section "general" "General Settings" "Configure basic options"
                    (ui/text "srv_name" "Server Name" "My Server" (table 'placeholder "Enter name"))
                    (ui/number "max_users" "Max Users" 100)
                    (ui/boolean "enabled" "Enabled" #t)
                    (ui/action-button "btn_save" "Save Changes" (table 'style "Primary" 'send-form #t))
                )
            )
        `;
        const page = anima.evaluatePage(script);
        expect(page).toBeInstanceOf(PageComponent);
        expect(page.components.length).toBe(2);

        const header = page.components[0] as UIDisplayElement;
        expect(header.compType).toBe("Display");
        expect(header.displayType).toBe("Header");
        expect(header.text).toBe("Server Configuration");

        const section = page.components[1] as SectionComponent;
        expect(section.compType).toBe("Section");
        expect(section.id).toBe("general");
        expect(section.title).toBe("General Settings");
        expect(section.entries.length).toBe(4);

        const textInput = section.entries[0] as UITextInput;
        expect(textInput.inputType).toBe("Text");
        expect(textInput.id).toBe("srv_name");
        expect(textInput.value).toBe("My Server");
        expect(textInput.opts.placeholder).toBe("Enter name");
    });

    it('manages FormSession with fast-path, memoized deps, and keyed reconciliation', () => {
        const anima = new SettingsAnima();

        // Dynamic formset script:
        // deps: ["mode"]
        // if mode == "advanced", renders extra field "advanced_key"
        const script = `
            (formset "my_form" (table 'deps '("mode"))
                (lambda (data)
                    (vector
                        (ui/text "title" "Title" (table-ref data "title" "Default Title"))
                        (ui/select "mode" "Mode" (table-ref data "mode" "basic") '(("basic" . "Basic") ("advanced" . "Advanced")))
                        (if (table-is? data "mode" "advanced")
                            (ui/text "advanced_key" "Advanced Key" (table-ref data "advanced_key" "Secret"))
                            ()
                        )
                        (ui/action-button "submit" "Submit")
                    )
                )
            )
        `;

        const formset = anima.engine.evaluateRaw(anima.engine.compileRaw(script)) as FormSetComponent;
        expect(formset).toBeInstanceOf(FormSetComponent);
        expect(formset.deps).toEqual(["mode"]);

        const listener = vi.fn();
        const initialData = Table.fromObject({ title: "Custom Title", mode: "basic" }, true);
        const session = anima.createFormSession(formset, initialData, listener);

        // Initial state
        expect(session.forms.length).toBe(2);
        const titleInputBefore = session.forms[0] as UITextInput;
        const modeInputBefore = session.forms[1] as UISelectInput;
        const submitActionBefore = session.actions[0];
        expect(titleInputBefore.value).toBe("Custom Title");
        expect(modeInputBefore.value).toBe("basic");
        expect(submitActionBefore.id).toBe("submit");

        // Test non-dependency field update (fast path, no structural change)
        const structural1 = session.setValue("title", "Updated Title");
        expect(structural1).toBe(false);
        expect(titleInputBefore.value).toBe("Updated Title");
        expect(session.getValue("title")).toBe("Updated Title");
        expect(listener).toHaveBeenCalledWith(session, {
            changedId: "title",
            value: "Updated Title",
            structural: false
        });
        expect(session.forms[0]).toBe(titleInputBefore); // Identity preserved!

        // Test dependency field update with SAME value (memoized dep check skips re-instantiation)
        listener.mockClear();
        const structural2 = session.setValue("mode", "basic");
        expect(structural2).toBe(false);
        expect(listener).toHaveBeenCalledWith(session, {
            changedId: "mode",
            value: "basic",
            structural: false
        });
        expect(session.forms[0]).toBe(titleInputBefore); // Identity preserved!

        // Test dependency field update with CHANGED value (structural change!)
        listener.mockClear();
        const structural3 = session.setValue("mode", "advanced");
        expect(structural3).toBe(true);
        expect(listener).toHaveBeenCalledWith(session, {
            changedId: "mode",
            value: "advanced",
            structural: true
        });

        // Now forms should have 3 fields: title, mode, advanced_key
        expect(session.forms.length).toBe(3);
        const titleInputAfter = session.forms[0] as UITextInput;
        const modeInputAfter = session.forms[1] as UISelectInput;
        const advInput = session.forms[2] as UITextInput;

        // Verify KEYED RECONCILIATION preserved existing instances!
        expect(titleInputAfter).toBe(titleInputBefore);
        expect(modeInputAfter).toBe(modeInputBefore);
        expect(session.actions[0]).toBe(submitActionBefore);

        // Verify updated values
        expect(titleInputAfter.value).toBe("Updated Title");
        expect(modeInputAfter.value).toBe("advanced");
        expect(advInput.id).toBe("advanced_key");
        expect(advInput.value).toBe("Secret");
        expect(advInput.session).toBe(session);
        expect(advInput.formset).toBe(formset);

        // Test setValue from component itself
        titleInputAfter.setValue("Direct Component Value");
        expect(session.getValue("title")).toBe("Direct Component Value");
    });
});
