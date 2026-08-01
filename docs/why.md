# Why Futuruna

I have pondered for a long time how to make a programming language that would
be able to encode the rule of law. Others have pondered the same, but by my
estimation, whatever has been made seems unmade for humans. I think that this
time it might be different.

Futuruna is not a programming language that ONLY encodes law; it is just a
programming language where this can be done naturally, in combination with
ordinary programming.

If you are a programming language nerd, which I am not, then you might have
tried [Prolog](https://www.swi-prolog.org/), which I have - and it's not easy.
It's innovative in what it achieves, it really is. You can describe rules, not
instructions, and the language itself becomes a search engine for answers.
This seems analogous to law. But it was never made for law in that way, and God
help you if you try - it's more of a language for database wizards. Again, the
language IS innovative, and Futuruna would not exist without it, but it does
not fit the bill.

Then we have a niche newcomer, one I am heavily inspired by: [Catala
Law](https://book.catala-lang.org/), just called Catala by its makers. Catala
really DOES encode laws as code. The problem with this language is that it ONLY
encodes law. Normal programming is outside of its domain and, in my opinion,
although I do not speak ill of the language's great ambition, it is really
difficult to use outside of strict law programming. And it's difficult to
integrate such a niche language with other ambitions.

Catala follows the natural way of writing laws, where you first describe the
default case of something, then patch on conditions and exceptions afterwards
that modify it. Normal programming is the other way around: you guard all the
special conditions up front until you get to the last line of the program,
which is the first line of the law. You should be able to choose your point of
entry here, and Futuruna allows that, with ergonomics I consider better than
Catala.

You might wonder now: why would anyone make a programming language today, when
LLMs can encode anything you want? The problem is that LLMs can only ever
encode things with the capabilities of the programming language they are
working with. If you try to make an LLM encode law without a language that
*can* encode law, the LLM will do its best to shoot itself in the foot, just so
you can keep spending tokens. You will be trying to make a bike somehow
speed-pedal to the Moon, although it cannot fly.

To make an LLM do more with code, you have to make the code able to do more. So
I did. Welcome to Futuruna. I hope you enjoy the language and start
experimenting yourself.

## The Quick Fly-In

Futuruna uses an innovative new approach where you (mostly) put a rune in FRONT
of the line you are about to write, which allows you to mix classical
programming with rule-based programming. I know, who would have thunk it,
right? Is it just a bad idea, and that's why no one has done it?

Well, my objection is that there is a simple reason why it is
actually a good idea. A front rune gives each kind of statement its own space,
so its capabilities do not have to compete with those in the language's other
spaces. This gives the author *more options* for writing a good program while
giving the reader *less uncertainty* about what they are reading. This can be
distilled into a fancy name: partitioned optionality, a solid tradeoff where
putting one character in front makes it easier to mix programming domains in
the same language. [The Philosophy of Futuruna](/research/philosophy) explores
the argument in more depth.

The seven runes and what they mean in Futuruna are explored in more depth
[here](/docs). All I will say is that I also believe Futuruna would work
exceptionally well with AI because of this front-rune system. One could
speculate that it would orient the latent space and attention heads of LLMs
more correctly toward the instruction ahead. Time will tell.

The last thing to say about Futuruna is that it is not made completely from
scratch. It transpiles to [Rust](https://rust-lang.org/), an extremely fast and
safe systems-level programming language. It removes several obstacles that
normal Rust beginners struggle with ([ownership
rules](/research/ownership)) without sacrificing safety, keeping Futuruna
readable and compatible with a wide variety of libraries, and of course making
Futuruna FAST.

Okay, maybe I am a programming language nerd, or have somewhat become one. I
invite you to [try out Futuruna](/playground): start encoding laws, contracts,
and documents, and audit them. Or start making programs and rules in a cool and
readable fashion. Let your imagination run wild and let Futuruna run with it.
