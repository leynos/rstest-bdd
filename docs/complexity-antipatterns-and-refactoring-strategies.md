# Navigating code complexity: a guide for implementers and maintainers

## 1. Introduction: the challenge of software complexity

Software development is an inherently complex endeavour. As systems evolve, and
as features are added, the intricacy of the codebase tends to increase, often
leading to challenges in maintenance, scalability, and developer productivity.
"Any fool can write code that a computer can understand. Good programmers write
code that humans can understand".[^1] This adage underscores a fundamental
truth: the long-term viability of a software project hinges significantly on
its comprehensibility and modifiability. Unchecked complexity can transform a
once-manageable system into a "full-blown algorithmic monster," torpedoing
performance and maintainability.[^2]

This report aims to equip code implementers and maintainers with a deeper
understanding of two critical complexity metrics—Cyclomatic Complexity and
Cognitive Complexity—and to explore a common manifestation of excessive
complexity known as the "Bumpy Road" antipattern. It will delve into strategies
for avoiding and rectifying this antipattern, examine its relationship with
fundamental software design principles like Separation of Concerns (SoC) and
architectural patterns such as Command Query Responsibility Segregation, and
finally, discuss clean refactoring approaches to reduce cognitive load without
inadvertently creating new forms of convoluted code. The ultimate goal is to
foster the development of software that is not only functional but also robust,
maintainable, and understandable for the long haul.

## 2. Understanding key complexity metrics

To effectively manage complexity, it is essential to measure it. Two prominent
metrics in software engineering are Cyclomatic Complexity and Cognitive
Complexity, each offering a distinct perspective on codebase complexity
challenges.

### A. Cyclomatic complexity: measuring testability

Cyclomatic Complexity, developed by Thomas J. McCabe, Sr. in 1976, is a
quantitative measure of the number of linearly independent paths through a
program's source code.[^3] It essentially quantifies the structural complexity
of a program by counting decision points that can affect the execution
flow.[^4] This metric is computed using the control-flow graph of the program,
where nodes represent indivisible groups of commands, and directed edges
connect nodes if one command can immediately follow another.[^3]

Cyclomatic Complexity is often expressed with the formula M=E−N+2P, where E is
the number of edges, N is the number of nodes, and P is the number of connected
components (typically 1 for a single program or method).[^3] A simpler
formulation applies to a single subroutine:

M = number of decision points + 1, where decision points include constructs
like `if` statements and conditional loops.[^3]

Thresholds and Implications:

High Cyclomatic Complexity indicates a more intricate control flow, which
directly impacts testability, maintainability, and defect discovery effort.[^1]
More paths mean more test cases are required for comprehensive coverage.[^4]
McCabe proposed the following risk categorization based on Cyclomatic
Complexity scores[^3]:

- 1-10: Simple procedure, little risk.

- 11-20: More complex, moderate risk.

- 21-50: Complex, high risk.

- 51+: Untestable code, very high risk.[^5]

  SonarQube suggests similar thresholds, with scores above 20 generally
  indicating a need for refactoring.[^1] While Cyclomatic Complexity helps
  assess how difficult code will be to test, the metric doesn't always align
  with how difficult it is for a human to understand.[^6]

### B. Cognitive complexity: measuring understandability

Cognitive Complexity, a metric notably championed by SonarSource, addresses a
different facet of code complexity: how difficult a piece of code is to
intuitively read and understand by a human.[^1] Unlike Cyclomatic Complexity,
which relies on mathematical graph theory, Cognitive Complexity aims to more
accurately reflect the mental effort required to comprehend the control flow of
a unit of code.[^7] It acknowledges that developers spend more time reading and
understanding code than writing it.[^8]

Core Principles of Calculation:

Cognitive Complexity is incremented based on three main rules[^8]:

1. **Breaks in Linear Flow:** Each time the code breaks the normal linear
   reading flow (e.g., loops, conditionals like
   `if`/`else`/`switch`, `try-catch` blocks, jumps to labels, and sequences of
   logical operators like `&&` and `||`), a penalty is applied.

2. **Nesting:** Each level of nesting of these flow-breaking structures adds an
   additional penalty. This is because deeper nesting makes it harder to keep
   the context in mind.

3. **Shorthand Discount:** Structures that allow multiple statements to be read
   as a single unit (e.g., a well-named method call) do not incur the same
   penalties as the raw statements they encapsulate. Method calls are generally
   "free" in terms of cognitive complexity, as a well-chosen name summarizes
   the underlying logic, allowing readers to grasp the high-level view before
   diving into details. However, recursive calls do increment the score.[^8]

For instance, a `switch` statement with multiple cases might have a high
Cyclomatic Complexity because each case represents a distinct path for testing.
However, if the structure remains straightforward, it stays easy to follow, so
its Cognitive Complexity might be relatively low.[^6] Conversely, deeply nested
logic, even with fewer paths, can significantly increase Cognitive Complexity
due to the mental effort required to track the conditions and context.[^9]

Thresholds and Implications:

Code with high Cognitive Complexity is harder to read, understand, test, and
modify.[^8] SonarQube, for example, raises issues when a function's Cognitive
Complexity exceeds a certain threshold, signaling that the code should likely
be refactored into smaller, more manageable pieces.[^8] The primary impact of
high Cognitive Complexity is a slowdown in development and an increase in
maintenance costs.[^8]

### Table 1: Cyclomatic vs. cognitive complexity

- **Primary Focus**
  - *Cyclomatic Complexity:* testability and execution paths
  - *Cognitive Complexity:* readability and human understanding
- **Basis**
  - *Cyclomatic Complexity:* rooted in graph theory
  - *Cognitive Complexity:* breaks in flow and nesting
- **Nesting Penalty**
  - *Cyclomatic Complexity:* counts paths only
  - *Cognitive Complexity:* adds cost for each nested level
- **Method Calls**
  - *Cyclomatic Complexity:* each path in a called method counts
  - *Cognitive Complexity:* generally free unless recursive
- **Logical Operators**
  - *Cyclomatic Complexity:* each condition is a decision point
  - *Cognitive Complexity:* mixed operators raise the score
- **Use Case Example**
  - *Cyclomatic Complexity:* high Cyclomatic Complexity, low cognitive load in
    simple switch
  - *Cognitive Complexity:* deep nesting raises cognitive load

Understanding both metrics provides a more holistic view of code quality. While
Cyclomatic Complexity guides testing efforts, Cognitive Complexity directly
addresses the human factor in software maintenance and evolution, making it a
crucial metric for sustainable software development.

## 3. The "bumpy road" antipattern

The "Bumpy Road" is a code smell that visually and structurally represents
functions or methods laden with excessive and poorly organized complexity.[^9]
Coined by Adam Tornhill, this antipattern describes code where the indentation
level oscillates, forming a lumpy visual pattern reminiscent of a bumpy road
when looking at the code's shape.[^9]

### A. Definition and characteristics

A method exhibiting the Bumpy Road antipattern typically contains multiple
sections, each characterized by deep nesting of conditional logic or loops.[^9]
Each "bump" in the road—a segment of deeply indented code—often signifies a
distinct responsibility, or even a separate logical chunk, that has not been
properly encapsulated.[^9]

Key characteristics include[^6]:

- **Multiple Chunks of Nested Logic:** The function isn't just deeply nested in
  one place, but has several such areas.

- **Visual Indentation Pattern:** The code's indentation creates a series of
  "hills and valleys."

- **Lack of Encapsulation:** Each bump often represents a missing abstraction or
  a responsibility that should have been extracted into its own function or
  method.

- **Increased Cognitive Load:** Navigating these bumps requires significant
  mental effort to keep track of the various conditions and states, taxing
  working memory.

- **Feature Entanglement:** In imperative languages, this structure increases
  the risk of feature entanglement, where different logical concerns become
  intertwined, leading to complex state management and a higher likelihood of
  defects.[^9]

The severity of a Bumpy Road can be assessed by[^6]:

- The depth of nesting within each bump (deeper is worse).

- The number of bumps (more bumps mean more missing abstractions and higher
  refactoring cost).

- The size (lines of code) of each bump (larger bumps are harder to mentally
  model).

Fundamentally, a Bumpy Road signifies a function that is trying to do too many
things, violating the Single Responsibility Principle. It acts as an obstacle
to comprehension, forcing developers to slow down, so they must pay meticulous
attention, much like a physical bumpy road slows down driving.[^9]

### B. How it forms and its impact

The Bumpy Road antipattern, like many software antipatterns, often emerges from
development practices that prioritize short-term speed over long-term
structural integrity.[^2] Rushed development cycles, lack of clear design, or
cutting corners on maintenance can lead to the gradual accumulation of
conditional logic within a single function.[^2] As new requirements emerge
alongside additional edge cases, developers might add conditional branches to
an existing method. Examples include an `if` statement, a loop, or a deeply
nested match added in haste when a team could instead step back to refactor and
create appropriate abstractions.

The impact of this antipattern is significant:

- **Reduced Readability and Understandability:** The convoluted structure makes
  it extremely difficult for developers to follow the logic and understand the
  method's overall purpose.[^9]

- **Increased Maintenance Costs:** Modifying that code, as well as debugging
  it, is time-consuming and error-prone. A change in one "bump" can have
  unintended consequences in another, especially if state is shared or
  manipulated across these logical chunks.[^2]

- **Higher Defect Rates:** The heavy tax on working memory and the risk of
  feature entanglement contribute to a higher likelihood of introducing
  bugs.[^9]

- **Impeded Evolvability:** Adding new features or adapting to changing
  requirements becomes a daunting task, as the existing complex structure
  resists modification.[^9]

- **Decreased Developer Productivity and Morale:** Continuously working with
  such code can be frustrating and demotivating.[^2]

The Bumpy Road is a strong predictor of code that is expensive to maintain and
risky to evolve.[^9] It's a clear signal that the code is not well-aligned with
how human brains process information, making it a prime candidate for
refactoring.

## 4. Navigating and rectifying the bumpy road

Addressing the Bumpy Road antipattern involves both proactive measures to
prevent its formation and reactive strategies to refactor existing problematic
code. Identifying early warning signs is also crucial.

### A. Avoiding the antipattern: proactive strategies

Preventing the Bumpy Road begins with a commitment to sound software
engineering principles from the outset.

1. **Adherence to the Single Responsibility Principle:** Ensure that each
   function, and each method, has one clear, well-defined responsibility.[^8]
   When a function handles multiple distinct logical blocks, decompose it
   immediately.

2. **Incremental Refactoring:** Don't wait for complexity to accumulate.
   Refactor code regularly as part of the development process, not as a
   separate, deferred task.[^10] "Make the change easy, and then make the easy
   change".[^11]

3. **Early Abstraction:** When a new piece of logic is being added, consider if
   it represents a distinct concept that warrants its own function or class.
   Well-named abstractions improve clarity.[^8]

4. **Code Reviews Focused on Structure:** Code reviews should not only check for
   correctness but also for structural integrity and complexity. Reviewers
   should look for emerging "bumps" or excessive nesting.

5. **Using Complexity Metrics:** Regularly monitor Cognitive Complexity scores
   using tools like SonarQube.[^1] Set thresholds and address violations
   promptly.

6. **Return Early / Guard Clauses:** To avoid deep nesting for validation or
   pre-condition checks, process exceptional cases first and return early.[^8]
   This approach flattens the main logic path and reduces nesting. For example,
   instead of:

   ```cpp
   void process(Data d) {
       if (d.isValid()) { // +1 (if)
           if (d.isReady()) { // +1 (if) +1 (nested)
               // main logic
           } else { // +1 (else)
               // handle not ready
           }
       } else { // +1 (else)
           // handle invalid
       }
   }

   ```

   Consider:

   ```cpp
   void process(Data d) {
       if (!d.isValid()) { // +1 (if)
           // handle invalid
           return;
       }
       if (!d.isReady()) { // +1 (if)
           // handle not ready
           return;
       }
       // main logic
   }

   ```

This approach significantly reduces nesting and, consequently, Cognitive
Complexity for the main execution path.

### B. Rectifying existing bumpy road code

Once a Bumpy Road is identified, the primary remediation strategy is the
**Extract Method** refactoring.[^9]

1. **Identify Logical Chunks:** Each "bump" or deeply nested section often
   corresponds to a specific sub-task or responsibility within the larger
   method.[^9]

2. **Extract to New Methods/Functions:** Encapsulate each identified chunk into
   its own well-named method or function.[^8] The name of the new method should
   clearly describe its purpose.

   - This breaks down the large, complex function into smaller, more manageable,
     and understandable pieces.[^8]

   - Even if the overall Cognitive Complexity of the program doesn't change
     significantly, the complexity is spread out, making individual functions
     easier to grasp.[^8]

3. **Parameterize Extracted Methods:** Pass necessary data to the new methods as
   parameters. Avoid relying on shared mutable state within the original class
   if possible, as this can maintain coupling.

4. **Iterative Refinement:** Refactoring complex code is often an iterative
   process. After initial extractions, further opportunities for simplification
   or abstraction may become apparent.[^10] Sometimes, extracting methods
   reveals that a more significant restructuring, perhaps involving new classes
   or design patterns (like the Command pattern for different actions within
   the bumps), is warranted.[^10]

Tools like CodeScene's Automatic Code Evolution (ACE) service can automatically
identify Bumpy Roads and even suggest or perform auto-refactoring for certain
languages.[^9]

### C. Red flags portending the bumpy road

Recognizing early warning signs can prevent minor complexity issues from
escalating into full-blown Bumpy Roads.

1. **Increasing Cognitive Complexity Scores:** A rising Cognitive Complexity
   score, as reported for a method in static analysis tools, is a direct
   indicator.[^8]

2. **Deeply Nested Logic:** Even a single area of deep nesting (more than 2–3
   levels) should be a concern. If multiple such areas appear in the same
   function, it's a strong red flag.[^9]

3. **Functions Doing "Too Much":** If describing what a function does requires
   using the word "and" multiple times (e.g., "it validates the input, and
   processes the data, and then updates the UI, and logs the result"), it's
   likely violating the Single Responsibility Principle and on its way to
   becoming bumpy.[^8]

4. **Frequent Modifications to the Same Function for Different Reasons:** If a
   function needs to be changed for various unrelated feature enhancements or
   bug fixes, it indicates it has too many responsibilities (related to the
   "Shotgun Surgery" code smell, which can be a consequence or co-occur with
   Bumpy Roads).[^2]

5. **Difficulty in Unit Testing:** If a method becomes hard to unit test due to
   numerous conditions and paths that need to be set up and verified, it often
   correlates with high complexity that could manifest as a Bumpy Road.

6. **Code "Smells" like Long Method:** A Bumpy Road is often, though not always,
   a Long Method.[^12] The length itself isn't the core problem, but it
   provides more space for bumps to accumulate.

7. **Declining Code Health Metrics:** Tools like CodeScene provide "Code Health"
   metrics which can degrade if Bumpy Roads are introduced.[^9]

By proactively addressing these red flags through disciplined refactoring,
teams can maintain a smoother, more navigable codebase.

## 5. Broader implications and clean refactoring approaches

High complexity and antipatterns like the Bumpy Road stem from violations of
fundamental software design principles. Understanding these connections, while
applying sophisticated refactoring techniques, is essential for building
maintainable systems.

### A. Relation to separation of concerns and command query responsibility segregation

1\. Separation of Concerns (SoC)

Separation of Concerns is a design principle that advocates for dividing a
computer program into distinct sections, where each section addresses a
separate concern.[^13] A "concern" is a set of information that affects the
code of a computer program. Modularity is achieved by encapsulating information
within a section of code that has a well-defined interface.[^13]

The Bumpy Road antipattern is a direct violation of SoC. Each "bump" in the
code often represents a distinct concern, or responsibility, that has been
improperly co-located within a single method.[^9] For example, a single method
might handle input validation, business logic processing for different cases,
data transformation, and error handling for each case, all intermingled.
Refactoring a Bumpy Road by extracting methods inherently applies SoC, as each
extracted method ideally handles a single, well-defined concern.[^10] This
leads to increased freedom for simplification, maintenance, module upgrade,
reuse, and independent development.[^13] While SoC might introduce additional
interfaces and require more code to execute, the accompanying coordination
overhead is typically outweighed by gains in clarity and maintainability,
especially as systems grow.[^13]

Consider a function that processes different types of user commands. A Bumpy
Road approach might have a large `if-else if-else` structure, with each block
handling a command type and its associated logic. This mixes the concern of
"dispatching" or "routing" based on command type with the concern of
"executing" each specific command. Applying SoC would involve separating these
concerns: one component decides which command to execute while separate
components—functions or classes—handle the execution of each command. This
separation makes the system easier to understand, test, and extend with new
commands.

2\. Command Query Responsibility Segregation

Command Query Responsibility Segregation is an architectural pattern that
segregates operations that modify state (commands) from operations that read
state (queries).[^14] Commands are task-based and should represent specific
business intentions (e.g.,

`BookHotelRoomCommand` rather than `SetReservationStatusCommand`).[^14] Queries
never alter data and return Data Transfer Objects optimized for display
needs.[^14]

While Command Query Responsibility Segregation operates at a higher
architectural level than a single Bumpy Road method, the principles are
related. Complex methods often arise when read and write concerns, or multiple
distinct command-like operations, are tangled together.

- **Preventing Bumpy Roads:** Applying Command Query Responsibility
  Segregation encourages developers to think about operations in terms of
  distinct commands and queries from the start. This naturally leads to
  smaller, more focused methods or handlers for each command and query,
  reducing the likelihood of a single method accumulating many "bumps" of
  unrelated logic.[^14] For instance, a method that both fetches data for a
  complex report, and then allows modifications based on that report, could
  become very complex. Command Query Responsibility Segregation would split
  this into a query to fetch the data and separate commands for any
  modifications.

- **Refactoring Complex Methods:** If a large method exhibits Bumpy Road
  characteristics because it handles multiple types of updates or decisions
  leading to state changes, Command Query Responsibility Segregation principles
  can guide its refactoring. The different "bumps" that correspond to different
  update logics could be refactored into separate command handlers.[^15] This
  aligns with the Single Responsibility Principle, as each command handler
  focuses on a single way of modifying state.[^15]

- **God Objects and Command Query Responsibility Segregation:** The "God
  Object" or "God Class" antipattern, where a single class hoards too much
  logic and responsibility, often leads to methods within that class becoming
  Bumpy Roads. Command Query Responsibility Segregation can help decompose God
  Objects by separating their command-handling responsibilities from their
  query-handling responsibilities, potentially leading to smaller, more focused
  classes (e.g., one class for command processing, another for query
  processing, or even finer-grained handlers).[^16] This separation simplifies
  each part, making them easier to manage and reducing the cognitive load
  associated with the original monolithic structure.

Command Query Responsibility Segregation promotes a clear separation that can
prevent the kind of tangled logic that forms Bumpy Roads. By isolating write
operations (commands) from read operations (queries), and by encouraging
task-based commands, the system naturally tends towards smaller, more cohesive
units of behaviour, thus reducing overall cognitive complexity within
individual components.[^14] The separation allows for independent optimization
and scaling of read and write sides, but more importantly for this discussion,
it enforces a structural discipline that discourages methods from accumulating
diverse responsibilities.[^14]

### B. Avoiding spaghetti code turning into ravioli code

When refactoring complex, tangled code (often called "Spaghetti Code"[^2]), a
common approach is to break it down into smaller pieces, such as helper
functions, focused classes, or dedicated modules. However, without careful
consideration for cohesion along with appropriate abstraction levels, this
approach can create "Ravioli Code".[^17] Ravioli Code consists of numerous
small, overly granular classes, functions, or traits where understanding the
overall programme flow requires navigating through many tiny, disconnected
pieces—making it as difficult to follow as the original spaghetti.[^17]

**Strategies to Avoid Ravioli Code:**

1. **Focus on Cohesion:** When extracting methods or creating new classes,
   ensure that the extracted code is functionally cohesive. Elements within a
   module (function or class) should be closely related and work together to
   achieve a single, well-defined purpose. Don't break down code arbitrarily
   based on length alone; base it on behaviour and meaningful abstractions.[^10]

2. **Balance Abstraction Levels:** Abstraction is about hiding unnecessary
   details and exposing essential features.[^18]

   - **Under-abstraction** (common in Spaghetti Code) leads to duplication and
     tight coupling.[^19]

   - **Over-abstraction** (risk in creating Ravioli Code) can make code harder
     to understand due to excessive layering and indirection, where simple
     operations are forced into complex object structures.[^17]

   - The key is to find the "right" level of abstraction that simplifies the
     problem domain without introducing unnecessary complexity. Create
     abstractions when painful duplication emerges or when a clear conceptual
     boundary can be established, not just for the sake of having more
     classes/objects.[^19] Start with simple, straightforward code and
     introduce abstractions only when genuinely needed.[^20]

3. **Meaningful Naming:** Clear and descriptive names for classes, methods, and
   variables are crucial, especially when dealing with many small components.
   Good names help convey the purpose and relationships between different parts
   of the code.[^10]

4. **Consider the "Why," Not Just the "How":** When refactoring, understand the
   underlying responsibilities and collaborations. Simply breaking code into
   smaller pieces without a clear architectural vision can lead to Ravioli.
   Design patterns, when applied appropriately, can provide a "system metaphor"
   or structure that makes the "ravioli" manageable by revealing symmetries and
   common sense in the design.[^21]

5. **Iterative Refactoring and Review:** Refactoring is not always a one-shot
   process. Continuously review the abstractions. Consider whether they help or
   hinder understanding, and whether too many trivial classes could be
   consolidated.[^10] Pair programming can also help maintain a balanced
   perspective during refactoring.[^21]

6. **The “Ya Ain't Gonna Need It” (YAGNI) principle:** The initialism
   summarizes the reminder that speculative features will probably go unused.
   It helps avoid unnecessary abstractions and features, which can contribute
   to Ravioli code if abstractions are created for anticipated but not actual
   needs.[^21]

7. **Focus on System Flow:** While individual components in Ravioli code might
   be simple, the difficulty lies in tracing the overall execution flow. Ensure
   that the interactions, and the dependencies between components, are clear
   and easy to follow. A slightly larger, more cohesive component often proves
   superior to many tiny ones when it improves overall system behaviour clarity.

The goal is not to have the fewest classes or methods, but to have a structure
where each component is easy to understand in isolation, and the interactions
between components are also clear and manageable. It's about finding a
"recursive Ravioli" structure, where at each level of containment, one deals
with a manageable number (e.g., 7 +/- 2) of components.[^21]

### C. Clean refactoring approaches to reduce cognitive complexity

Several specific refactoring techniques can be particularly effective in
reducing cognitive complexity, especially when dealing with conditional logic
and method structure.

### Table 2: Refactoring approaches for reducing cognitive complexity

- **Balanced Abstraction (e.g., Extract Method)**
  - Break large methods into smaller, cohesive units
  - Benefit: shorter methods and clearer intent
  - Solves spaghetti code and Bumpy Road issues
- **Structural pattern matching**
  - Replace complex if/else or switch constructs with pattern matching
  - Benefit: simpler conditional logic and data extraction
  - Solves deeply nested conditionals
- **Declarative Programming**
  - Focus on what to achieve instead of how to do it
  - Benefit: less state tracking and clearer intent
  - Solves imperative loops and manual state management
- **Dispatcher/Command Pattern**
  - Encapsulate actions in objects and route via a dispatcher
  - Benefit: removes large conditional blocks
  - Solves complex switch statements

#### 1. Structural pattern matching

Structural pattern matching—available in languages like Python (since 3.10 with
match-case) and C#—offers a declarative and expressive way to handle complex
conditional logic, often replacing verbose if-elif-else chains or switch
statements.[^21]

It works by allowing code to match against the *structure* of data—such as its
type, shape, or specific values within sequences like lists or tuples, or
mappings such as dictionaries—and simultaneously destructure this data, binding
parts of it to variables.[^22] This approach can significantly reduce cognitive
load. The clarity comes from the direct mapping of data shapes to code blocks,
making it easier to understand the conditions under which a piece of code
executes.[^23] For instance, instead of multiple `isinstance` checks followed
by key lookups and value comparisons in a nested `if` structure to parse a JSON
object, a single `case` statement with a mapping pattern can define the
expected structure and extract the necessary values concisely.[^22] This shifts
the focus from an imperative sequence of checks to a declarative description of
data shapes, which is often more intuitive. The destructuring capability is
particularly powerful, as it eliminates the manual code otherwise needed to
extract values after a condition has been met, reducing boilerplate while also
cutting the number of mental steps a developer must follow.[^22]

Consider processing different event types from a UI framework, where events are
represented as dictionaries.[^24]

- *Imperative (Python-like pseudocode):*

```python
  event_data = get_event()
  if "type" in event_data and event_data["type"] == "click":
      if "position" in event_data and isinstance(event_data["position"], tuple) and len(event_data["position"]) == 2:
          x, y = event_data["position"]
          handle_click(x, y)
  elif "type" in event_data and event_data["type"] == "keypress":
      if "key_name" in event_data:
          key = event_data["key_name"]
          handle_keypress(key)
  #… and so on for other event types

   ```

- *Declarative with Structural Pattern Matching (Python* `match-case`*):*

  ```python
  event_data = get_event()
  match event_data:
      case {"type": "click", "position": (x, y)}: # Matches structure and extracts x, y
          handle_click(x, y)
      case {"type": "keypress", "key_name": key}: # Matches structure and extracts key
          handle_keypress(key)
      case _:
          handle_unknown_event()

  ```

The pattern matching version is more readable and directly expresses the
expected structure of each event type, reducing the cognitive effort to
understand the conditions and data extraction. Key features like guards (`if`
conditions on `case` statements) allow for additional non-structural checks,
further enhancing its power.[^22]

#### 2. Embracing declarative programming

Declarative programming focuses on describing what result is desired, rather
than detailing how to achieve it step-by-step, as is typical in imperative
programming.[^25] This paradigm shift can significantly reduce cognitive
complexity by abstracting away low-level control flow and state management.

When developers write declarative code, they operate at a higher level of
abstraction, allowing them to reason about the program's intent more
directly.[^25] This often leads to more concise, readable, and maintainable
code because the "noise" of explicit iteration, temporary variables, and manual
state updates is minimized.[^25] Many declarative approaches also inherently
favour immutability, reduce side effects, and encourage deterministic
behaviour—common culprits for bugs and increased cognitive load in imperative
code.[^26]

Examples include using Structured Query Language for database queries—
specifying the desired dataset rather than the retrieval algorithm[^34]—or
employing functional programming constructs like `map`, `filter`, and `reduce`
on collections instead of writing explicit loops. Refactoring imperative code
to a declarative style can start small, perhaps by converting a loop that
filters and transforms a list into a chain of `filter` and `map`
operations.[^26] The broader adoption of declarative approaches in areas like
UI development (e.g., React) and data querying signifies an industry trend
towards managing complexity by raising abstraction levels. However, the
effectiveness of declarative programming relies on well-designed underlying
abstractions; a poorly designed declarative layer might not successfully hide
complexity or could introduce its own.[^27]

#### 3. Employing dispatcher and command patterns

For managing complex conditional logic that selects different behaviours (often
found in Bumpy Roads or large switch statements), these complementary patterns
offer a structured and extensible alternative.

The **Command pattern** encapsulates a request or an action as an object.[^28]
Each command object implements a common interface (e.g., with an

`execute()` method). This decouples the object that invokes the command from
the object that knows how to perform it. Instead of a large conditional
checking a type and then executing logic, different command objects can be
instantiated based on the type, and then their `execute()` method is called.
This promotes the Single Responsibility Principle, as each command class
handles a single action, making the system easier to test, extend, and
evolve.[^29]

The **Dispatcher pattern** often works in conjunction with the Command pattern.
A dispatcher is a central component that receives requests (which could be
command objects or simple identifiers) and routes them to the appropriate
handler.[^29] For instance, a

`switch` statement where each `case` calls a different method can be refactored
by creating an interface for handlers, a concrete handler class for each
original `case`, and a dispatcher (perhaps a map from case identifiers to
handler instances) that looks up and invokes the correct handler.[^30] This
transforms the control flow from a monolithic conditional block into a more
manageable registration and lookup mechanism. The cognitive load is reduced
because developers can focus on individual, self-contained handlers while
trusting the dispatcher's routing logic.

For example, a `switch` statement handling different message types:

```java
// Before
void handleMessage(Message msg) {
    switch (msg.getType()) {
        case "TYPE_A":
            processTypeA(msg);
            break;
        case "TYPE_B":
            processTypeB(msg);
            break;
        //… more cases
        default:
            handleUnknown(msg);
    }
}
```

Can be refactored using a dispatcher and command/handler objects:

```java
// After
interface MessageHandler {
    void handle(Message msg);
}
class TypeAHandler implements MessageHandler {
    public void handle(Message msg) { /* processTypeA logic */ }
}
class TypeBHandler implements MessageHandler {
    public void handle(Message msg) { /* processTypeB logic */ }
}
//… other handlers

class MessageDispatcher {
    private Map<String, MessageHandler> handlers = new HashMap<>();
    public MessageDispatcher() {
        handlers.put("TYPE_A", new TypeAHandler());
        handlers.put("TYPE_B", new TypeBHandler());
        //… register other handlers
    }
    public void dispatch(Message msg) {
        MessageHandler handler = handlers.getOrDefault(msg.getType(), this::handleUnknown);
        if (handler!= null) {
            handler.handle(msg);
        }
    }
    private void handleUnknown(Message msg) { /*… */ }
}
```

This approach not only simplifies the original `handleMessage` method but also
makes the system more extensible, as new message types can be supported by
adding new handler classes and registering them with the dispatcher, often
without modifying existing dispatcher code (aligning with the Open/Closed
Principle). However, it's important to ensure that the dispatch mechanism
itself remains clear and that the proliferation of small classes doesn't lead
to Ravioli Code, where the overall system flow becomes obscured.[^17] Maintain
clear naming conventions, and ensure the logical organization remains
consistent.[^31]

The **State pattern** is a related behavioural pattern useful when an object's
behaviour changes depending on its internal state.[^32] Instead of using large
conditionals based on state variables, each state is encapsulated in its own
object. The context object delegates behaviour to its current state object.
Transitions involve changing the context's state object. This is particularly
effective for refactoring state machines implemented with complex

`if/else` or `switch` statements.[^32]

Thoughtfully apply these refactoring strategies to significantly reduce
cognitive complexity, and to create codebases that are more understandable,
maintainable, and adaptable to future changes.

## 6. Conclusion: towards a more maintainable and understandable codebase

Managing software complexity, particularly the cognitive load it imposes on
developers, is not a one-time task but a continuous discipline crucial for the
long-term health and success of any software project.[^11] This report has
explored Cyclomatic and Cognitive Complexity as vital metrics for quantifying
different aspects of this challenge, with Cognitive Complexity offering a more
nuanced view of human understandability. The Bumpy Road antipattern serves as a
clear indicator of localized, excessive complexity, often stemming from
violations of the Single Responsibility Principle and a lack of timely
refactorings.

The journey towards a more maintainable codebase involves recognizing such
antipatterns and understanding their connection to fundamental principles like
Separation of Concerns. Architectural choices, such as adopting Command Query
Responsibility Segregation, can also play a significant role in structuring
systems to naturally avoid the entanglement of responsibilities that leads to
high complexity in individual components.

Ultimately, writing clean code—code that is easy to read, understand, and
modify—is paramount.[^2] This is achieved not merely through aesthetic choices
but through deliberate design and refactoring efforts. Techniques such as
ensuring balanced abstraction, leveraging structural pattern matching for
clearer conditional logic, embracing declarative programming paradigms, and
employing patterns like the Dispatcher pattern alongside the Command pattern
can transform convoluted code into more manageable and comprehensible
structures. However, these techniques must be applied judiciously, always
prioritizing genuine improvements in clarity and maintainability over adherence
to a pattern for its own sake, to avoid pitfalls like Ravioli Code.

Integrate these principles and techniques into daily development practices
through a proactive and disciplined approach. This includes regular code
reviews, monitoring complexity metrics, and fostering a team culture that
values code quality and continuous improvement. The oft-quoted wisdom, "Good
programmers write code that humans can understand"[^1], remains the guiding
principle. Strive for this ideal. That focus delivers systems that are
powerful, efficient, and genuinely enjoyable for the team to evolve and
maintain.

## Works cited

[^1]: How to Identify and Reduce Cognitive Complexity in a Codebase — Axify,
      <https://axify.io/blog/cognitive-complexity>
[^2]: Top 5 Software Anti Patterns to Avoid for Better Development Outcomes |
      BairesDev, <https://www.bairesdev.com/blog/software-anti-patterns/>
[^3]: Cyclomatic complexity — Wikipedia,
      <https://en.wikipedia.org/wiki/Cyclomatic_complexity>
[^4]: Cyclomatic complexity: Definition and limits in understanding code
      quality — the Developer Experience platform,
      <https://getdx.com/blog/cyclomatic-complexity/>
      <https://getdx.com/blog/cyclomatic-complexity/>
[^5]: Cyclomatic complexity — Wikipedia (risk classification),
      <https://en.wikipedia.org/wiki/Cyclomatic_complexity>
[^6]: Cognitive Complexity — Code Climate,
      <https://docs.codeclimate.com/docs/cognitive-complexity>
[^7]: Cognitive Complexity | Sonar SonarSource | Sonar,
      <https://www.sonarsource.com/resources/cognitive-complexity/#:~:text=This%20paper%20describes%20Cognitive%20Complexity,Complexity%20precedents%20with%20human%20assessment.>
[^8]: Cognitive Complexity of functions should not be too high — Rules -
      SonarQube Server,
      <https://next.sonarqube.com/sonarqube/coding_rules?languages=cpp&q=cognitive&open=cpp%3AS3776>
[^9]: The Bumpy Road Code Smell: Measuring Code Complexity by its Shape and
      Distribution,
      <https://codescene.com/engineering-blog/bumpy-road-code-complexity-in-context/>
[^10]: Bumpy Road — Samman Technical Coaching,
      <https://sammancoaching.org/code_smells/bumpy_road.html>
[^11]: Adam Tornhill, *Code as a Crime Scene*, Second Edition,
       <https://media.pragprog.com/titles/atcrime2/logic.pdf>
[^12]: The software antipatterns that are killing development speed | Okoone,
       <https://www.okoone.com/spark/strategy-transformation/the-software-anti-patterns-that-are-killing-development-speed/>
[^13]: How to *resist* refactoring a large spaghetti codebase? :
       r/SoftwareEngineering — Reddit,
       <https://www.reddit.com/r/SoftwareEngineering/comments/11smgtp/how_to_resist_refactoring_a_large_spaghetti/>
[^14]: Refactoring nested conditional statements — Software Engineering
       Stack Exchange,
       <https://softwareengineering.stackexchange.com/questions/47789/>
[^15]: CodeScene Automatic Code Evolution (ACE): Auto-Refactor Code,
       <https://codescene.io/docs/auto-refactor/index.html>
[^16]: Code Smells — Samman Technical Coaching,
       <https://sammancoaching.org/reference/code_smells/>
[^17]: src/sas/sascalc/poresize/maxEnt_method.py — CodeScene,
       <https://codescene.io/projects/64300/delta?repo-id=263464&review-id=3247&biomarker=Overall+Code+Complexity&filename=src%2Fsas%2Fsascalc%2Fporesize%2FmaxEnt_method.py&method=>
[^18]: Separation of concerns — Wikipedia,
       <https://en.wikipedia.org/wiki/Separation_of_concerns>
[^19]: Command Query Responsibility Segregation pattern — Azure Architecture
       Center | Microsoft Learn,
       <https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs>
[^20]: Mastering Command Query Responsibility Segregation: 7 Powerful Benefits,
       <https://hyscaler.com/insights/mastering-cqrs-benefits-of-command-query/>
[^21]: Implementing Command Query Responsibility Segregation in the dotnet
       ecosystem — Dev Community,
       <https://dev.to/moh_moh701/enhancing-net-applications-with-cqrs-and-solid-principles-30i8>
[^22]: Refactoring a God class — Stack Overflow,
       <https://stackoverflow.com/questions/14870377/>
[^23]: Command Query Responsibility Segregation: Understanding From First
       Principles — NDepend Blog,
       <https://blog.ndepend.com/cqrs-understanding-first-principles/>
[^24]: When to use the Command Query Responsibility Segregation design
       pattern? — architecture — Stack Overflow,
       <https://stackoverflow.com/questions/8820748/when-to-use-the-cqrs-design-pattern>
[^25]: Another pasta-themed programming problem is "ravioli code". That …,
       <https://news.ycombinator.com/item?id=4537664>
[^26]: Ravioli Code — C2 wiki, <https://wiki.c2.com/?RavioliCode>
[^27]: Ravioli code — why an antipattern? — Stack Overflow,
       <https://stackoverflow.com/questions/2052017/ravioli-code-why-an-anti-pattern>
[^28]: A Guide to Data Abstraction and Its Significant Benefits — CelerData,
       <https://celerdata.com/glossary/a-guide-to-data-abstraction>
[^29]: The Role of Abstraction in Software Development | Bebras Armenia,
       <https://bebras.am/en/blog/The-Role-of-Abstraction-in-Software-Development>
[^30]: Abstraction, Refactoring, Complexity, and Tradeoffs — Part 1 | Synth
       …, <https://omux.dev/blog/abstraction-refactoring-complexity/>
[^31]: Don't create over abstractions — Castineiras thoughts -,
       <https://www.castineiras.com/?p=102>
[^32]: Refactor `if-else` Statements to `match-case` for Improved Readability
       and Maintainability in Python 3.10+ · Issue #453 — GitHub,
       <https://github.com/sourcery-ai/sourcery/issues/453>
