# Anima Language

Anima is the custom (Scheme-inspired) language used in settings v2 in antiraid for dynamic branching + complex client-side validation etc.

## Specification


# Deviations from Scheme

## No first-class continuations

In order to keep Anima simple to implement (and debug!), Anima does not support full first-class continuations *yet* (such as ``call-with-current-continuation`` or ``call/cc``) (although support for this may be implemented later at some point in the future). This also enables for potential future optimizations.