# Pathfinding in Rust on "A *Map"

Writing a pathfinding project is a perennial personal programming exercise for me. 

Doing an implementation that works on the Eve Online map is just something relatively comfortable 
(there's plenty of size to it, but it's also familiar and there's opportunity to try interesting things).
My first implementation of A* against Eve Online's starmap was nearly 20 years ago in C++ as a demo, and the SDE representation
of the Eve Universe is (probably still) in a form that I created (probably ~2012ish?).

Be a bit gentle, it's the largest and most complex Rust codebase I've created so far, and often I did things just to try them.

The A* implementation isn't perfect:
It should reject new nodes that are for the same location that are higher heuristic than we already have.

Note that there are a number of things in here that are mostly for the value of learning and experimentation in Rust.

## What's fun about this?

### It can stream and load the SDE from the internet

Enable the "download" feature and don't have a path set.
I disabled this by default because it doesn't save the zip locally and is therefore quite wasteful of your bandwidth.

Go download it and run it against that: https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/sde.zip

### It is abstracted over "Cost"

You can use integers or floats as your "cost". Floats need wrapping with `NotNan<_>` to be `Ord` but are perfectly valid.

### It uses Rayon to load and decompress all the universe YAML files
It's fun. You can watch all your cores suddenly spike.

### It uses Serde to load the YAML
At various times it used `Value` but now uses types to unmarshal the YAML data.

### It is abstracted over the storage

The Open and Closed "lists" are abstracted from the algorithm by closures.  
This makes it easy to change and improve the storage.

### There are a load of ways to represent a "[Star]Map"

I spent a lot of time trying different ones.  

Some observations:
* A map in this context is unchanging once created
* Creating a map/graph usually involves multiple passes to create links after you've created nodes
  * OnceCell tended to be the strongest restriction you could put on it, was convenient enough
* All nodes have the lifetime of the top level map
* Should really be able to be shared across multiple concurrent/parallel pathfind operations on different cores
* It would be entirely acceptable to build a whole new map and swap it in to handle changes

#### Vec and offsets
What I eventually used. What pretty much everyone points you at.
Somewhat unsatisfying, it's a reference you just don't tell the compiler about to make it safer.

Made this a little more fun by wrapping the index offsets in a newtype and implementing `Index`.

#### [A]Rc<>s and Weak references
Unsatisfying because you have an essentially static "map" structure and you're paying a runtime cost. 
Once you want to share that map on different threads: Arc/OnceCell etc.

#### Arenas
Was tempting: All items have the lifetime of the parent Arena. Cache locality in the allocators.
Most arenas like `Bumpalo` seem to be `!Sync`, so you can't just plop it in a top level `Rc<>` as non-mut to ensure the lifetime 
lives with the objects.

You could probably wrap the arena in an object that makes it `Sync` by never allowing access.

#### Vec and references
In theory, the lifetime of references to items in a `Vec` is the same as the lifetime of the `Vec` and are all the same.
Sort of worked. Felt jinky, and I wasn't quite sure when Rust was going to start complaining.

#### Pointers
Worked. `Box` the items, `Arc` the whole map. Do some pinning, a bit of unsafe. Might work. Might blow up.
Passed Miri.

All the nodes could be all over the heap. Maybe there's a good way to do this, but wasn't massively motivated to explore it for too long.

