# Settings (v2) protocol:

## Base Types not listed in this file

export type DispatchResult<T> = {
    type: "ok" | "err",
    id: string, -- template's id
    value: T, -- value within it
} 

## Format

All settings v2 payloads are events sent via msyscall's Bot->DispatchEvent op with the following base properties:

- `name`: WebSettings
- `author`: Author of the user invoking the setting (msyscall will auto-fill this in when using DispatchEvent)
- `data`: Data of type `Event` as shown on this page. Like all DispatchEvent requests, all data must follow the KhronosValue format

# Basic architecture

There are a few things any settings v2 client must architecturally support:

- The frontend must keep track of which forms are in a formset as well as the order of forms within a formset
- The frontend must have some way to keep track of the formdata (``{[string]: {FormData}}``) hereby called frontend-formdata
- The frontend must have some way to apply a formdata patchset from the server (hereby called formdata-patchset) to the frontend-formdata
- The template id each page was sent in must also be tracked and will hereby be called page-template-id.

## Frontend

To start, a fetch_page Event must be sent which will return a DispatchResult<Page>[], the frontend can then parse+render the comps however it wants 
(see willow in template-worker devutils/willow) [TODO: improve this section, also add note that initial frontend-formdata should include all fields w/o branch
evaluation]. Note that every FormElement that is not a DisplayElement must have a unique id to all other form elements (including form elements in branches)

### Choices

A Text and/or Array.Text input may contain an optional `choices` property that may act as a *hint* to the frontend on how to render the Text/Array.Text input.

For example, a Text with a choices of type=Fixed may be rendered as a select box. Similarly, a Array.Text with a choices of type=Fixed may be rendered with a 
custom MultiSelect component or a HTML select multiple.

Note: it is perfectly allowed for a frontend to *downgrade* choices it does not support (or cannot render) to the default rendering for Text/Array.Text. 
For example, a type=Channel/type=Role in a frontend without fetched guild info etc may be rendered as a raw text box. The choice=nil option is
the bare minimum required when rendering Text/Array.Text.

Some other specifications for individual types:

- type=Channel: Should cover all *non-category* channels

### Form Actions

If a user clicks an action button (FormAction with type Button.Event), follow the below steps to create the form_action Event to send:

If send_form is true on the action button, create the form_data object with the following rules:
- All elements (FormElement) within the currently open form must be looped through. 
- If the element is a DisplayElements, ignore,
- If the element is a Branch, then compute the branch using Anima w/ disableLambda+disableDefine and if it evaluates to a truthy value, then recursively 
loop over the FormElements of the branch and follow the same rules listed here on those elements using the same in-creation form_data.
- Otherwise, form_data[FormElement.id] = FormElement.value

Set __tloop_template_id to page-template-id
Set action_button_id to the id of the action button the user pressed
Set formset_id to the id of the formset containing the form the action button was contained in
Set form_id to the id of the form the action button was contained in

The result of a form_action Event is either a any response (todo: improve this) or a error (seen as a msyscall error) as tloop will perform selective dispatch
due to special logic included for the __tloop_template_id key and will directly call into the desired template w/o any DispatchResult intermediaries etc.

## Anima

Anima is the language used by form branches and validation in settings v2. It is designed based of s-expressions to be fairly minimal. 

See ``../anima`` for the exact specification of ``anima``