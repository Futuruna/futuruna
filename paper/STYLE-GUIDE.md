# Style Guide: The Futuruna Paper

## The Reader

Smart, curious, no physics or math degree. Willing to work through hard ideas. Not willing to be lost, talked down to, or lectured at. Trusts the author when the author earns it through clear reasoning, not when the author asserts it.

**The mortal human test:** At every technical passage, ask: would a thoughtful 30-year-old who reads widely but never took physics past high school still be with me? If the answer is no, the passage needs a ladder. The reader should never have to Google a term to keep reading.

## Voice

The paper is written by a person who found something, not a professor delivering a lecture. The tone is: clear, honest, precise, occasionally wry. Explain everything; condescend about nothing.

**Model authors:** Carl Sagan (build every concept from where the reader already stands), Carlo Rovelli (warmth + precision), Annie Dillard (spend it all now), Richard Feynman (puzzle before explanation).

---

## The Concept Ladder

The hardest concepts in the paper (transition matrix, entropy, eigenvalue, integrated information, causal entropic forces) cannot be introduced by name. They must be *built* from something the reader already holds.

Every hard concept follows three steps:

1. **Image** -- a concrete scene the reader can picture
2. **Plain English** -- what is happening, in words a teenager would follow
3. **Name** -- "Mathematicians call this X" or "This is what physicists mean by X"
4. **Symbol** (optional) -- the notation, presented as shorthand for what they already understand

The name always comes last. The reader should be nodding before they learn what the thing is called.

### The Hard Concepts and Their Ladders

| Concept | Ladder from... |
|---------|----------------|
| **Transition matrix** | For each token in a programming language, a table of what can come next and how often it does. A table of one-step predictions. |
| **Shannon entropy** | How spread out your options are. If you could end up at any of 20 places with roughly equal odds, your options are wide open (high entropy). If you always end up at the same 3 places, your options are narrow (low entropy). |
| **S_tau (causal path entropy)** | The diversity of your three-token futures. From where you are in the code, how many meaningfully different continuations exist? |
| **Jensen-Shannon divergence** | How distinguishable are two positions? Can you tell whether you're inside a type declaration or a function body just from the local context? |
| **Principal components** | If you measure a thousand data points and they all fall along a thin line, there's really only one dimension -- the others are redundant. PCA counts the independent directions that carry real information. |
| **Integrated information (Phi)** | How much the system is more than the sum of its parts. If knowing one axis tells you nothing about the others, integration is high. |
| **d_eff (effective dimensionality)** | How many independent channels of information the syntax provides at once. Two instruments that always agree are one dimension. Three that vary independently are three. |

### The Reminder Phrase

When a concept reappears after a gap, attach a brief reminder. Not a full re-derivation -- a phrase.

**Examples:**
- "S_tau (the diversity of reachable continuations)"
- "Phi (integrated information -- how much the system exceeds the sum of its parts)"
- "the transition matrix (the table of one-step token predictions)"

These reminders cost 5-10 words and save the reader from flipping back.

---

## The Sagan Principle: Prose Carries the Argument

If you deleted every equation and every symbol from the paper, the reader should still understand the argument. Equations confirm what the prose already established. They are windows, not walls.

**The test:** Cover the equation with your hand. Does the surrounding text make the point on its own? If not, the text is leaning on the math instead of the other way around.

---

## The Six Literary Techniques

### 1. Build the Ladder (Sagan)

Never name a concept before the reader understands it. Start from where *they* stand, not where the concept lives.

**Rule:** No technical term appears on the page until the reader can already picture what it refers to.

### 2. Body First (Sagan + Ariely)

Start with the body, not the concept. Put the reader in a physical situation before naming the physics. They should *feel* the principle before they *learn* it.

**Rule:** Every hard concept should be preceded by something the reader can feel, see, or remember from their own experience.

### 3. Spend It All Now (Dillard)

Don't save the best insight for later. Don't build up with throat-clearing. The killer line goes in the first three sentences.

### 4. Crowd and Leap (Le Guin)

Pack the image tight, then jump. No transitions like "This leads us to consider..." Trust the reader to follow a hard cut.

### 5. The Invisible Skeleton (McPhee)

Structure the reader never sees but always feels. Recurring motifs, callbacks. Never say "As we discussed in Section 2..." Just use the image. The reader remembers.

### 6. Aim Past the Wood (Dillard)

Don't write *at* the reader. Write *through* them. The paper is not explaining a theory to an audience. It is following a thread and inviting the reader to watch. Prefer "The equation says..." over "I will now show you that..."

---

## Anti-Patterns

### JARGON BOMB
A technical term appears with no preparation. Fix: introduce the *experience* first, then the name.

### PARENTHETICAL DEFINITION
A term is defined inside parentheses mid-sentence. Fix: promote to its own sentence or cut it. If a parenthetical contains more than 3 words of definition, it needs its own sentence.

### LECTURE MODE
A numbered or bulleted list of formal definitions. Fix: weave into narrative prose.

### SPEED VIOLATION
A concept is introduced and immediately used in a chain of reasoning. Fix: one new technical concept per paragraph. Let each one breathe.

### DENSITY WALL
A paragraph with 5+ sentences of continuous technical content and no narrative relief. Fix: break in two. Insert a concrete image or analogy.

### TABLE WITHOUT SETUP
A data table appears with no narrative preparation. Fix: before any table, one sentence that says what the reader should look for.

---

## Rules

### 1. The First-Use Rule

Every technical term gets a plain-English definition on first use. No exceptions.

### 2. Confidence Without Defensiveness

The math is real. The simulations run. State the claim, show the evidence, let the reader decide.

**Delete entirely:**
- "Not a metaphor." / "Not as an analogy." / "Literally."
- "This should stop you in your tracks."
- "Here is the key insight."
- "Note that" / "It is important to realize"

**The test:** If a sentence tells the reader what to feel ("this is surprising"), delete the instruction and let the content do the work.

### 3. Sentence Hygiene

**Em-dashes:** Maximum one per paragraph.

**"Not X. Y." construction:** Maximum twice in the entire paper.

**Parenthetical asides:** Convert most to their own sentences. Parentheses signal the author could not decide whether to include something. Decide.

**Sentence length:** Vary deliberately.

### 4. Bring Them Along

The reader should arrive at each conclusion a half-beat before the text states it. Build the scenario, walk through the steps, let the result emerge. Then name it.

### 5. Show the Work

Walk through reasoning before naming the result. Tables and equations get a sentence before (what you are about to see) and a sentence after (what you just saw).

---

## The Standard

The reader should:
- Never need to re-read a sentence to understand it
- Never encounter a term they haven't been prepared for
- Feel they are being told a story, not taught a course
- Be able to explain the main idea to someone else in one sentence

If a passage fails any of these, it needs work.
